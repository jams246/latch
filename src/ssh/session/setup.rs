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
    memory: &mut super::Memory,
    reconnecting: bool,
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
        keepalive_interval: Some(Duration::from_secs(3)),
        keepalive_max: 2,
        ..Default::default()
    };
    let connecting = async {
        let (socket, guard) =
            crate::ssh::socket::connect(&config.host, config.port, closed).await?;
        let handle = client::connect_stream(Arc::new(settings), socket, client).await?;
        Ok::<_, Error>((handle, guard))
    };
    let (mut handle, guard) = tokio::time::timeout(
        Duration::from_secs(if reconnecting { 5 } else { 180 }),
        connecting,
    )
    .await
    .map_err(|_| Error::Timeout)??;
    if !reconnecting {
        events
            .send(Event::Status(State::Authenticating))
            .await
            .map_err(|_| Error::Cancelled)?;
    }
    tokio::select! {
        result = crate::ssh::auth::authenticate(&mut handle, config, prompts, &mut memory.credential) => result?,
        _ = disconnected.changed() => return Err(Error::Transport),
    }
    let mut initial = Vec::new();
    let shell = if request_shell {
        let detected = super::probe::detect(&handle).await;
        let mut shell = open_terminal(&handle, config, &mut initial).await?;
        if let Some(detected) = detected {
            shell
                .exec(true, detected.command(memory.directory.path()))
                .await?;
            match await_success(&mut shell, &mut initial).await {
                Ok(()) => (),
                Err(Error::ChannelRejected) => {
                    let _ = shell.close().await;
                    initial.clear();
                    shell = open_terminal(&handle, config, &mut initial).await?;
                    shell.request_shell(true).await?;
                    await_success(&mut shell, &mut initial).await?;
                }
                Err(error) => return Err(error),
            }
        } else {
            shell.request_shell(true).await?;
            await_success(&mut shell, &mut initial).await?;
        }
        Some(shell)
    } else {
        if reconnecting {
            tokio::time::timeout(Duration::from_secs(5), async {
                let session = russh_sftp::client::SftpSession::new(open_sftp(&handle).await?)
                    .await
                    .map_err(|_| Error::Transport)?;
                let _ = session.close().await;
                Ok::<_, Error>(())
            })
            .await
            .map_err(|_| Error::Timeout)??;
        }
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

async fn open_terminal(
    handle: &client::Handle<Client>,
    config: &ConnectionConfig,
    initial: &mut Vec<u8>,
) -> Result<Channel<client::Msg>, Error> {
    let mut channel = handle.channel_open_session().await?;
    channel
        .request_pty(true, "xterm-256color", config.cols, config.rows, 0, 0, &[])
        .await?;
    await_success(&mut channel, initial).await?;
    Ok(channel)
}

async fn await_success(
    channel: &mut Channel<client::Msg>,
    initial: &mut Vec<u8>,
) -> Result<(), Error> {
    const MAX_INITIAL_OUTPUT: usize = 1024 * 1024;
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Success) => return Ok(()),
            Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                if data.len() > MAX_INITIAL_OUTPUT.saturating_sub(initial.len()) {
                    return Err(Error::ChannelOutputLimit);
                }
                initial.extend_from_slice(&data);
            }
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
    await_success(&mut channel, &mut Vec::new()).await?;
    Ok(channel.into_stream())
}
