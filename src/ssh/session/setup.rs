use std::{sync::Arc, time::Duration};

use russh::{Channel, ChannelMsg, client};
use tokio::sync::{mpsc, watch};

use crate::ssh::{
    ConnectionConfig, Error, Event, Prompt, SftpStream, State,
    host::{CancelOnDrop, Client},
    socket::SocketGuard,
};

pub(super) struct Established {
    pub handle: client::Handle<Client>,
    pub shell: Option<Channel<client::Msg>>,
    pub initial: Vec<u8>,
    pub _socket_guard: SocketGuard,
    pub disconnected: watch::Receiver<bool>,
}

pub(super) async fn establish(
    config: &ConnectionConfig,
    prompts: &mpsc::Sender<Prompt>,
    events: &mpsc::Sender<Event>,
    request_shell: bool,
) -> Result<Established, Error> {
    let (cancel, cancelled) = watch::channel(false);
    let (closed, mut disconnected) = watch::channel(false);
    let _cancel_on_drop = CancelOnDrop(cancel);
    let client = Client {
        host: config.host.clone(),
        port: config.port,
        path: config.known_hosts.clone(),
        prompts: prompts.clone(),
        cancel: cancelled,
        closed: closed.clone(),
        disconnected: disconnected.clone(),
    };
    let settings = client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        ..Default::default()
    };
    let (socket, guard) = crate::ssh::socket::connect(&config.host, config.port, closed).await?;
    let mut handle = client::connect_stream(Arc::new(settings), socket, client).await?;
    events
        .send(Event::Status(State::Authenticating))
        .await
        .map_err(|_| Error::Cancelled)?;
    tokio::select! {
        result = crate::ssh::auth::authenticate(&mut handle, config, prompts) => result?,
        _ = disconnected.changed() => return Err(Error::Transport),
    }
    let mut initial = Vec::new();
    let shell = if request_shell {
        let mut shell = handle.channel_open_session().await?;
        shell
            .request_pty(true, "xterm-256color", config.cols, config.rows, 0, 0, &[])
            .await?;
        initial = await_success(&mut shell).await?;
        shell.request_shell(true).await?;
        initial.extend(await_success(&mut shell).await?);
        Some(shell)
    } else {
        None
    };
    Ok(Established {
        handle,
        shell,
        initial,
        _socket_guard: guard,
        disconnected,
    })
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

pub(super) async fn open_sftp(handle: &client::Handle<Client>) -> Result<SftpStream, Error> {
    let mut channel = handle.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    await_success(&mut channel).await?;
    Ok(channel.into_stream())
}
