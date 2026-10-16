use std::{sync::Arc, time::Duration};

mod setup;

use russh::ChannelMsg;
use tokio::sync::mpsc;

use super::{Command, ConnectionConfig, Error, Event, Prompt, State};
use setup::{Established, establish, open_sftp};

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

/// Drives a session on a caller-owned Tokio task, emitting its final lifecycle state.
/// Closing the command channel cancels the attempt; callers must abort the task on drop.
pub async fn run(
    config: ConnectionConfig,
    commands: mpsc::Receiver<Command>,
    prompts: mpsc::Sender<Prompt>,
    events: mpsc::Sender<Event>,
) {
    run_session(config, commands, prompts, events, true).await;
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
    let setup = tokio::time::timeout(
        Duration::from_secs(180),
        establish(config, prompts, events, request_shell),
    );
    tokio::pin!(setup);
    let mut size = (config.cols, config.rows);
    let Established {
        handle,
        mut shell,
        initial,
        _socket_guard,
        mut disconnected,
    } = loop {
        tokio::select! {
            result = &mut setup => break result.map_err(|_| Error::Timeout)??,
            command = commands.recv() => match command {
                Some(Command::Disconnect) | None => return Err(Error::Cancelled),
                Some(Command::OpenSftp(reply)) => { let _ = reply.send(Err(Error::Authentication)); },
                Some(Command::Resize { cols, rows }) if cols > 0 && rows > 0 => size = (cols, rows),
                _ => (),
            }
        }
    };
    if size != (config.cols, config.rows)
        && let Some(shell) = &shell
    {
        shell.window_change(size.0, size.1, 0, 0).await?;
    }
    events
        .send(Event::Status(State::Connected))
        .await
        .map_err(|_| Error::Cancelled)?;
    if !initial.is_empty() {
        events
            .send(Event::Output(initial))
            .await
            .map_err(|_| Error::Cancelled)?;
    }
    let handle = Arc::new(handle);
    let mut channel_requests = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Input(data)) => {
                    if let Some(shell) = &shell { shell.data(data.as_slice()).await?; }
                }
                Some(Command::Resize { cols, rows }) if cols > 0 && rows > 0 => {
                    if let Some(shell) = &shell { shell.window_change(cols, rows, 0, 0).await?; }
                }
                Some(Command::OpenSftp(reply)) => {
                    let handle = handle.clone();
                    channel_requests.spawn(async move {
                        let result = tokio::time::timeout(Duration::from_secs(20), open_sftp(&handle))
                            .await.map_err(|_| Error::Timeout).and_then(|result| result);
                        let _ = reply.send(result);
                    });
                }
                Some(Command::Disconnect) | None => break,
                _ => (),
            },
            _ = channel_requests.join_next(), if !channel_requests.is_empty() => (),
            _ = disconnected.changed(), if shell.is_none() => return Err(Error::Transport),
            message = async { shell.as_mut()?.wait().await }, if shell.is_some() => match message {
                Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                    events.send(Event::Output(data.to_vec())).await.map_err(|_| Error::Cancelled)?;
                }
                Some(ChannelMsg::ExitStatus { exit_status }) => {
                    let _ = events.send(Event::Exit(exit_status)).await;
                }
                Some(ChannelMsg::Close | ChannelMsg::Eof) | None => break,
                _ => (),
            }
        }
    }
    if let Some(shell) = shell {
        let _ = shell.close().await;
    }
    tokio::time::timeout(
        Duration::from_secs(5),
        handle.disconnect(russh::Disconnect::ByApplication, "Session closed", "en"),
    )
    .await
    .map_err(|_| Error::Timeout)??;
    Ok(())
}
