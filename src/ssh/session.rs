use std::{sync::Arc, time::Duration};

use russh::{Channel, ChannelMsg, client};
use tokio::sync::mpsc;

use super::{Command, ConnectionConfig, Error, Event, Prompt, SftpStream, State, host::Client};

pub struct SessionControl {
    pub commands: mpsc::Sender<Command>,
    task: tokio::task::JoinHandle<()>,
}

struct SocketGuard(std::net::TcpStream);

impl Drop for SocketGuard {
    fn drop(&mut self) {
        // russh detaches its handshake task before returning a session handle.
        let _ = self.0.shutdown(std::net::Shutdown::Both);
    }
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
    let (handle, mut shell, initial, _socket_guard) = loop {
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
    let handle = Arc::new(handle);
    let mut channel_requests = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Input(data)) => shell.data(data.as_slice()).await?,
                Some(Command::Resize { cols, rows }) if cols > 0 && rows > 0 => {
                    shell.window_change(cols, rows, 0, 0).await?;
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
) -> Result<
    (
        client::Handle<Client>,
        Channel<client::Msg>,
        Vec<u8>,
        SocketGuard,
    ),
    Error,
> {
    let (cancel, cancelled) = tokio::sync::watch::channel(false);
    let (closed, mut disconnected) = tokio::sync::watch::channel(false);
    let _cancel_on_drop = super::host::CancelOnDrop(cancel);
    let client = Client {
        host: config.host.clone(),
        port: config.port,
        path: config.known_hosts.clone(),
        prompts: prompts.clone(),
        cancel: cancelled,
        closed,
    };
    let settings = client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    };
    let socket = tokio::time::timeout(
        Duration::from_secs(20),
        tokio::net::TcpStream::connect((config.host.as_str(), config.port)),
    )
    .await
    .map_err(|_| Error::Timeout)?
    .map_err(|_| Error::Transport)?;
    socket.set_nodelay(true).map_err(|_| Error::Transport)?;
    let socket = socket.into_std().map_err(|_| Error::Transport)?;
    let guard = SocketGuard(socket.try_clone().map_err(|_| Error::Transport)?);
    let socket = tokio::net::TcpStream::from_std(socket).map_err(|_| Error::Transport)?;
    let mut handle = client::connect_stream(Arc::new(settings), socket, client).await?;
    events
        .send(Event::Status(State::Authenticating))
        .await
        .map_err(|_| Error::Cancelled)?;
    tokio::select! {
        result = super::auth::authenticate(&mut handle, config, prompts) => result?,
        _ = disconnected.changed() => return Err(Error::Transport),
    }
    let mut shell = handle.channel_open_session().await?;
    shell
        .request_pty(true, "xterm-256color", config.cols, config.rows, 0, 0, &[])
        .await?;
    let mut initial = await_success(&mut shell).await?;
    shell.request_shell(true).await?;
    initial.extend(await_success(&mut shell).await?);
    Ok((handle, shell, initial, guard))
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
