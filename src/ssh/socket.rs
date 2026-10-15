use std::time::Duration;

use tokio::{
    io::{DuplexStream, copy, duplex, split},
    net::TcpStream,
    sync::watch,
    task::JoinHandle,
};

use super::Error;

pub(super) struct SocketGuard {
    socket: std::net::TcpStream,
    task: JoinHandle<()>,
    closed: watch::Sender<bool>,
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = self.closed.send(true);
        // russh can still be awaiting a handler callback when its caller cancels.
        let _ = self.socket.shutdown(std::net::Shutdown::Both);
        self.task.abort();
    }
}

pub(super) async fn connect(
    host: &str,
    port: u16,
    closed: watch::Sender<bool>,
) -> Result<(DuplexStream, SocketGuard), Error> {
    let socket = tokio::time::timeout(Duration::from_secs(20), TcpStream::connect((host, port)))
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::Transport)?;
    socket.set_nodelay(true).map_err(|_| Error::Transport)?;
    let socket = socket.into_std().map_err(|_| Error::Transport)?;
    let retained = socket.try_clone().map_err(|_| Error::Transport)?;
    let socket = TcpStream::from_std(socket).map_err(|_| Error::Transport)?;
    let (stream, bridge) = duplex(64 * 1024);
    let notify = closed.clone();
    let task = tokio::spawn(async move {
        let (mut incoming, mut outgoing) = socket.into_split();
        let (mut requests, mut responses) = split(bridge);
        // Read EOF independently while russh awaits explicit trust in its key callback.
        // Either direction ending closes this whole SSH transport, including half-closes.
        tokio::select! {
            _ = copy(&mut incoming, &mut responses) => (),
            _ = copy(&mut requests, &mut outgoing) => (),
        }
        let _ = notify.send(true);
    });
    Ok((
        stream,
        SocketGuard {
            socket: retained,
            task,
            closed,
        },
    ))
}
