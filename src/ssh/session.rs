use std::{sync::Arc, time::Duration};

use russh::{Channel, ChannelMsg, client};
use tokio::sync::mpsc;

use super::{Command, ConnectionConfig, Error, Event, Prompt, SftpStream, State, host::Client};

pub struct SessionControl {
    pub commands: mpsc::Sender<Command>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for SessionControl {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn spawn(
    config: ConnectionConfig,
    prompts: mpsc::Sender<Prompt>,
    events: mpsc::Sender<Event>,
) -> SessionControl {
    let (commands, receiver) = mpsc::channel(128);
    let task = tokio::spawn(run(config, receiver, prompts, events));
    SessionControl { commands, task }
}

pub async fn run(
    config: ConnectionConfig,
    mut commands: mpsc::Receiver<Command>,
    prompts: mpsc::Sender<Prompt>,
    events: mpsc::Sender<Event>,
) {
    let result = serve(&config, &mut commands, &prompts, &events).await;
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
) -> Result<(), Error> {
    if config.host.is_empty()
        || config
            .host
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "#,|[]".contains(c))
        || config.port == 0
        || config.username.is_empty()
        || config.username.contains('\0')
        || config.cols == 0
        || config.rows == 0
    {
        return Err(Error::InvalidConfig);
    }
    events
        .send(Event::Status(State::Connecting))
        .await
        .map_err(|_| Error::Cancelled)?;
    let setup = tokio::time::timeout(Duration::from_secs(180), establish(config, prompts, events));
    tokio::pin!(setup);
    let mut size = (config.cols, config.rows);
    let (handle, mut shell, initial) = loop {
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
    if size != (config.cols, config.rows) {
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
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Input(data)) => shell.data(data.as_slice()).await?,
                Some(Command::Resize { cols, rows }) if cols > 0 && rows > 0 => {
                    shell.window_change(cols, rows, 0, 0).await?;
                }
                Some(Command::OpenSftp(reply)) => {
                    let result = tokio::time::timeout(Duration::from_secs(20), open_sftp(&handle))
                        .await.map_err(|_| Error::Timeout).and_then(|result| result);
                    let _ = reply.send(result);
                }
                Some(Command::Disconnect) | None => break,
                _ => (),
            },
            message = shell.wait() => match message {
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
    let _ = shell.close().await;
    handle
        .disconnect(russh::Disconnect::ByApplication, "Session closed", "en")
        .await?;
    Ok(())
}

async fn establish(
    config: &ConnectionConfig,
    prompts: &mpsc::Sender<Prompt>,
    events: &mpsc::Sender<Event>,
) -> Result<(client::Handle<Client>, Channel<client::Msg>, Vec<u8>), Error> {
    let (cancel, cancelled) = tokio::sync::watch::channel(false);
    let _cancel_on_drop = super::host::CancelOnDrop(cancel);
    let client = Client {
        host: config.host.clone(),
        port: config.port,
        path: config.known_hosts.clone(),
        prompts: prompts.clone(),
        cancel: cancelled,
    };
    let settings = client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    };
    let mut handle = client::connect(
        Arc::new(settings),
        (config.host.as_str(), config.port),
        client,
    )
    .await?;
    events
        .send(Event::Status(State::Authenticating))
        .await
        .map_err(|_| Error::Cancelled)?;
    super::auth::authenticate(&mut handle, config, prompts).await?;
    let mut shell = handle.channel_open_session().await?;
    shell
        .request_pty(true, "xterm-256color", config.cols, config.rows, 0, 0, &[])
        .await?;
    let mut initial = await_success(&mut shell).await?;
    shell.request_shell(true).await?;
    initial.extend(await_success(&mut shell).await?);
    Ok((handle, shell, initial))
}

async fn await_success(channel: &mut Channel<client::Msg>) -> Result<Vec<u8>, Error> {
    let mut initial = Vec::new();
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Success) => return Ok(initial),
            Some(ChannelMsg::Data { data }) => initial.extend_from_slice(&data),
            Some(ChannelMsg::Failure | ChannelMsg::Close | ChannelMsg::Eof) | None => {
                return Err(Error::ChannelRejected);
            }
            _ => (),
        }
    }
}

async fn open_sftp(handle: &client::Handle<Client>) -> Result<SftpStream, Error> {
    let mut channel = handle.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    await_success(&mut channel).await?;
    Ok(channel.into_stream())
}
