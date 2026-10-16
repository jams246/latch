use std::{sync::Arc, time::Duration};

use russh::ChannelMsg;
use tokio::sync::mpsc;

use super::setup::{Established, open_sftp};
use crate::ssh::{Command, ConnectionConfig, Error, Event, State};

pub(super) async fn serve(
    connection: Established,
    config: &mut ConnectionConfig,
    commands: &mut mpsc::Receiver<Command>,
    events: &mpsc::Sender<Event>,
    memory: &mut super::Memory,
) -> Result<(), Error> {
    let Established {
        handle,
        mut shell,
        initial,
        _socket_guard,
        mut disconnected,
    } = connection;
    events
        .send(Event::Status(State::Connected))
        .await
        .map_err(|_| Error::Cancelled)?;
    if !initial.is_empty() {
        memory.directory.feed(&initial);
        events
            .send(Event::Output(initial))
            .await
            .map_err(|_| Error::Cancelled)?;
    }
    let handle = Arc::new(handle);
    let mut channel_requests = tokio::task::JoinSet::new();
    let mut exited = false;
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Input(data)) => {
                    if let Some(shell) = &shell { shell.data(data.as_slice()).await?; }
                }
                Some(Command::Resize { cols, rows }) if cols > 0 && rows > 0 => {
                    config.cols = cols;
                    config.rows = rows;
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
                Some(Command::Reconnect) => return Err(Error::Transport),
                Some(Command::Disconnect) | None => break,
                _ => (),
            },
            _ = channel_requests.join_next(), if !channel_requests.is_empty() => (),
            _ = disconnected.changed(), if shell.is_none() => return Err(Error::Transport),
            message = async { shell.as_mut()?.wait().await }, if shell.is_some() => match message {
                Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                    memory.directory.feed(&data);
                    events.send(Event::Output(data.to_vec())).await.map_err(|_| Error::Cancelled)?;
                }
                Some(ChannelMsg::ExitStatus { exit_status }) => {
                    exited = true;
                    let _ = events.send(Event::Exit(exit_status)).await;
                }
                Some(ChannelMsg::Close) | None if exited => break,
                Some(ChannelMsg::Close) | None => return Err(Error::Transport),
                _ => (),
            }
        }
    }
    if let Some(shell) = shell {
        let _ = shell.close().await;
    }
    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        handle.disconnect(russh::Disconnect::ByApplication, "Session closed", "en"),
    )
    .await;
    Ok(())
}
