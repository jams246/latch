use std::time::Duration;

use tokio::{sync::mpsc, time::Instant};

use super::Memory;
use super::setup::{Established, establish};
use crate::ssh::{Command, ConnectionConfig, Error, Event, Prompt};

pub(super) async fn attempt(
    config: &mut ConnectionConfig,
    commands: &mut mpsc::Receiver<Command>,
    prompts: &mpsc::Sender<Prompt>,
    events: &mpsc::Sender<Event>,
    request_shell: bool,
    memory: &mut Memory,
    reconnect_deadline: Option<Instant>,
) -> Result<Established, Error> {
    let reconnecting = reconnect_deadline.is_some();
    let deadline = reconnect_deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(180));
    let starting = config.clone();
    let setup = tokio::time::timeout_at(
        deadline,
        establish(
            &starting,
            prompts,
            events,
            request_shell,
            memory,
            reconnecting,
        ),
    );
    tokio::pin!(setup);
    let connection = loop {
        tokio::select! {
            biased;
            result = &mut setup => break result.map_err(|_| Error::Timeout)??,
            command = commands.recv() => pending(command, config)?,
        }
    };
    loop {
        match commands.try_recv() {
            Ok(command) => pending(Some(command), config)?,
            Err(mpsc::error::TryRecvError::Disconnected) => return Err(Error::Cancelled),
            Err(mpsc::error::TryRecvError::Empty) => break,
        }
    }
    if (config.cols, config.rows) != (starting.cols, starting.rows)
        && let Some(shell) = &connection.shell
    {
        tokio::time::timeout_at(
            deadline,
            shell.window_change(config.cols, config.rows, 0, 0),
        )
        .await
        .map_err(|_| Error::Timeout)??;
    }
    Ok(connection)
}

pub(super) async fn recover(
    config: &mut ConnectionConfig,
    commands: &mut mpsc::Receiver<Command>,
    prompts: &mpsc::Sender<Prompt>,
    events: &mpsc::Sender<Event>,
    request_shell: bool,
    memory: &mut Memory,
) -> Result<Established, Error> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut error = Error::Transport;
    loop {
        if Instant::now() >= deadline {
            return Err(error);
        }
        match attempt(
            config,
            commands,
            prompts,
            events,
            request_shell,
            memory,
            Some(deadline),
        )
        .await
        {
            Ok(connection) => return Ok(connection),
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(failure) => error = failure,
        }
        let retry = tokio::time::sleep_until(deadline.min(Instant::now() + Duration::from_secs(1)));
        tokio::pin!(retry);
        loop {
            tokio::select! {
                biased;
                _ = &mut retry => break,
                command = commands.recv() => pending(command, config)?,
            }
        }
    }
}

fn pending(command: Option<Command>, config: &mut ConnectionConfig) -> Result<(), Error> {
    match command {
        Some(Command::Disconnect) | None => return Err(Error::Cancelled),
        Some(Command::OpenSftp(reply)) => {
            let _ = reply.send(Err(Error::Authentication));
        }
        Some(Command::Resize { cols, rows }) if cols > 0 && rows > 0 => {
            config.cols = cols;
            config.rows = rows;
        }
        _ => (),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::{AuthMethod, fixture::Fixture};

    #[tokio::test(start_paused = true)]
    async fn unavailable_server_retries_until_exactly_thirty_seconds() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let mut config = ConnectionConfig {
            host: "127.0.0.1".into(),
            port,
            username: "synthetic".into(),
            auth: AuthMethod::Password,
            known_hosts: std::path::PathBuf::new(),
            cols: 80,
            rows: 24,
        };
        let (_commands, mut receiver) = mpsc::channel(8);
        let (prompts, _requests) = mpsc::channel(8);
        let (events, mut output) = mpsc::channel(8);
        let before = Instant::now();
        assert!(
            recover(
                &mut config,
                &mut receiver,
                &prompts,
                &events,
                true,
                &mut Memory::default()
            )
            .await
            .is_err()
        );
        assert_eq!(Instant::now() - before, Duration::from_secs(30));
        assert!(output.try_recv().is_err());
    }

    #[tokio::test]
    async fn reconnect_discards_pending_input_and_preserves_resize() {
        let fixture = Fixture::start(false).await;
        let mut config = crate::ssh::tests::config(&fixture, "recovery-pending-input");
        russh::keys::known_hosts::learn_known_hosts_path(
            &config.host,
            config.port,
            crate::ssh::fixture::key(1).public_key(),
            &config.known_hosts,
        )
        .unwrap();
        let (commands, mut receiver) = mpsc::channel(8);
        let (prompts, mut requests) = mpsc::channel(8);
        let (events, _output) = mpsc::channel(8);
        commands
            .send(Command::Input(b"must not replay".to_vec()))
            .await
            .unwrap();
        commands
            .send(Command::Resize {
                cols: 110,
                rows: 41,
            })
            .await
            .unwrap();
        let mut memory = Memory {
            credential: Some(crate::ssh::Secret::new("test-only-password".into())),
            ..Memory::default()
        };
        let connection = attempt(
            &mut config,
            &mut receiver,
            &prompts,
            &events,
            true,
            &mut memory,
            Some(Instant::now() + Duration::from_secs(30)),
        )
        .await
        .unwrap();
        assert_eq!((config.cols, config.rows), (110, 41));
        assert!(receiver.try_recv().is_err());
        assert!(requests.try_recv().is_err());
        drop(connection);
    }
}
