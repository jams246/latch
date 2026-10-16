use std::future::Future;

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::watch,
};

use super::{Error, transfer_progress::Reporter};

pub(super) async fn copy<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    source: &mut R,
    destination: &mut W,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
    progress: &mut Reporter,
) -> Result<(), Error> {
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        wait_running(cancel, paused).await?;
        let length = cancellable(cancel, source.read(&mut buffer)).await?;
        if length == 0 {
            cancellable(cancel, destination.flush()).await?;
            return Ok(());
        }
        cancellable(cancel, destination.write_all(&buffer[..length])).await?;
        progress.advance(length);
        tokio::task::yield_now().await;
    }
}

pub(super) async fn wait_running(
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
) -> Result<(), Error> {
    loop {
        check_cancel(cancel)?;
        if !*paused.borrow_and_update() {
            return Ok(());
        }
        tokio::select! {
            _ = cancel.changed() => return Err(Error::Cancelled),
            changed = paused.changed() => {
                changed.map_err(|_| Error::Cancelled)?;
            }
        }
    }
}

async fn cancellable<T>(
    cancel: &mut watch::Receiver<bool>,
    operation: impl Future<Output = std::io::Result<T>>,
) -> Result<T, Error> {
    check_cancel(cancel)?;
    tokio::select! {
        _ = cancel.changed() => Err(Error::Cancelled),
        result = operation => result.map_err(Error::Local),
    }
}

pub(super) fn check_cancel(cancel: &watch::Receiver<bool>) -> Result<(), Error> {
    if *cancel.borrow() || cancel.has_changed().is_err() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}
