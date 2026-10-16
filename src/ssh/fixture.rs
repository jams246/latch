use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use russh::{
    keys::{PrivateKey, ssh_key},
    server,
};
use tokio::net::TcpListener;

use super::fixture_handler::Handler;

#[derive(Default)]
pub(crate) struct Counts {
    pub passwords: AtomicUsize,
    pub challenge_responses: AtomicUsize,
    pub active: AtomicUsize,
    pub ptys: AtomicUsize,
    pub shells: AtomicUsize,
    pub subsystems: AtomicUsize,
    pub resizes: AtomicUsize,
    pub agent_forwarding: AtomicUsize,
    pub cols: AtomicUsize,
    pub rows: AtomicUsize,
    pub execs: Mutex<Vec<String>>,
    pub directory: Mutex<Option<String>>,
    pub reject_wrapped_shell: AtomicBool,
}

pub(crate) fn key(seed: u8) -> PrivateKey {
    let pair = ssh_key::private::Ed25519Keypair::from_seed(&[seed; 32]);
    PrivateKey::new(pair.into(), "synthetic test key").unwrap()
}

pub(crate) struct Fixture {
    pub port: u16,
    pub counts: Arc<Counts>,
    shutdown: server::RunningServerHandle,
    task: tokio::task::JoinHandle<()>,
}

pub(super) type Subsystem = fn(russh::ChannelStream<server::Msg>);

impl Fixture {
    pub fn disconnect_clients(&self) {
        self.shutdown.shutdown("Synthetic transport failure".into());
    }

    pub async fn start(reject_shell: bool) -> Self {
        Self::start_with(reject_shell, None).await
    }

    pub async fn start_with(reject_shell: bool, subsystem: Option<Subsystem>) -> Self {
        Self::start_options(reject_shell, false, subsystem, None).await
    }

    pub async fn start_sftp(subsystem: Subsystem) -> Self {
        Self::start_options(true, true, Some(subsystem), None).await
    }

    pub async fn start_shell(shell: &'static str) -> Self {
        Self::start_shell_with(shell, None).await
    }

    pub async fn start_shell_with(shell: &'static str, subsystem: Option<Subsystem>) -> Self {
        Self::start_options(false, false, subsystem, Some(shell)).await
    }

    async fn start_options(
        reject_shell: bool,
        reject_pty: bool,
        subsystem: Option<Subsystem>,
        shell: Option<&'static str>,
    ) -> Self {
        use server::Server as _;
        let socket = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = socket.local_addr().unwrap().port();
        let config = Arc::new(server::Config {
            keys: vec![key(1)],
            auth_rejection_time: std::time::Duration::ZERO,
            auth_rejection_time_initial: Some(std::time::Duration::ZERO),
            ..Default::default()
        });
        let counts = Arc::new(Counts::default());
        let mut server = Factory {
            counts: counts.clone(),
            reject_shell,
            reject_pty,
            subsystem,
            shell,
        };
        let (ready, receiver) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let running = server.run_on_socket(config, &socket);
            let _ = ready.send(running.handle());
            let _ = running.await;
        });
        let shutdown = receiver.await.unwrap();
        Self {
            port,
            counts,
            shutdown,
            task,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.shutdown.shutdown("Synthetic fixture stopped".into());
        self.task.abort();
    }
}

struct Factory {
    counts: Arc<Counts>,
    reject_shell: bool,
    reject_pty: bool,
    subsystem: Option<Subsystem>,
    shell: Option<&'static str>,
}

impl server::Server for Factory {
    type Handler = Handler;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Handler {
        self.counts.active.fetch_add(1, Ordering::SeqCst);
        Handler {
            counts: self.counts.clone(),
            reject_shell: self.reject_shell,
            reject_pty: self.reject_pty,
            channels: HashMap::new(),
            subsystem: self.subsystem,
            flood_output: false,
            shell: self.shell,
            directory: "/home/synthetic".into(),
        }
    }
}
