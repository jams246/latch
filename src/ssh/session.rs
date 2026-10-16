mod connected;
mod directory;
mod probe;
mod recovery;
mod setup;
pub(super) mod shell;

use tokio::sync::mpsc;

use super::{Command, ConnectionConfig, Error, Event, Prompt, State};

#[derive(Default)]
pub(super) struct Memory {
    credential: Option<super::Secret>,
    directory: directory::Directory,
}

/// Owns one tab's transport task; dropping it aborts pending work and closes its socket.
pub struct SessionControl {
    /// Commands for this session; input is never shared with another tab.
    pub commands: mpsc::Sender<Command>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for SessionControl {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Starts an independent session on the currently entered Tokio runtime.
/// Retain the returned owner for the tab's lifetime and consume both UI channels.
pub fn spawn(
    config: ConnectionConfig,
    prompts: mpsc::Sender<Prompt>,
    events: mpsc::Sender<Event>,
) -> SessionControl {
    spawn_mode(config, prompts, events, true)
}

/// Starts an authenticated transport without requesting a PTY or shell.
/// After [`State::Connected`], open SFTP with [`Command::OpenSftp`].
pub fn spawn_sftp(
    config: ConnectionConfig,
    prompts: mpsc::Sender<Prompt>,
    events: mpsc::Sender<Event>,
) -> SessionControl {
    spawn_mode(config, prompts, events, false)
}

fn spawn_mode(
    config: ConnectionConfig,
    prompts: mpsc::Sender<Prompt>,
    events: mpsc::Sender<Event>,
    request_shell: bool,
) -> SessionControl {
    let (commands, receiver) = mpsc::channel(128);
    let task = tokio::spawn(run_session(
        config,
        receiver,
        prompts,
        events,
        request_shell,
    ));
    SessionControl { commands, task }
}

async fn run_session(
    config: ConnectionConfig,
    mut commands: mpsc::Receiver<Command>,
    prompts: mpsc::Sender<Prompt>,
    events: mpsc::Sender<Event>,
    request_shell: bool,
) {
    let result = serve(&config, &mut commands, &prompts, &events, request_shell).await;
    match result {
        Ok(()) | Err(Error::Cancelled) => {
            let _ = events.send(Event::Status(State::Disconnected)).await;
        }
        Err(error) => {
            let _ = events.send(Event::Error(error.to_string())).await;
            let _ = events.send(Event::Status(State::Failed)).await;
        }
    }
}

async fn serve(
    config: &ConnectionConfig,
    commands: &mut mpsc::Receiver<Command>,
    prompts: &mpsc::Sender<Prompt>,
    events: &mpsc::Sender<Event>,
    request_shell: bool,
) -> Result<(), Error> {
    if config.host.is_empty()
        || config
            .host
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "#,|[]".contains(c))
        || config.port == 0
        || config.username.is_empty()
        || config.username.contains('\0')
        || (request_shell && (config.cols == 0 || config.rows == 0))
    {
        return Err(Error::InvalidConfig);
    }
    events
        .send(Event::Status(State::Connecting))
        .await
        .map_err(|_| Error::Cancelled)?;
    let mut config = config.clone();
    let mut memory = Memory::default();
    let mut connection = recovery::attempt(
        &mut config,
        commands,
        prompts,
        events,
        request_shell,
        &mut memory,
        None,
    )
    .await?;
    loop {
        let result = connected::serve(connection, &mut config, commands, events, &mut memory).await;
        match result {
            Ok(()) | Err(Error::Cancelled) => return result,
            Err(_) => {
                memory.directory.reset_parser();
                events
                    .send(Event::Status(State::Reconnecting))
                    .await
                    .map_err(|_| Error::Cancelled)?;
                connection = recovery::recover(
                    &mut config,
                    commands,
                    prompts,
                    events,
                    request_shell,
                    &mut memory,
                )
                .await?;
            }
        }
    }
}
