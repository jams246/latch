use std::{future::Future, sync::Arc, time::Duration};

use russh_sftp::client::SftpSession;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{mpsc, watch},
};

use super::{
    Direction, Error, TransferEvent,
    transfer_files::{download, upload},
    transfer_progress::Reporter,
    transfer_trees::copy_many,
};

pub(super) enum Sources {
    File(Direction),
    Selection(Vec<Direction>),
}

/// Cooperative transfer cancellation, including file closure when its owning tab closes.
/// Await [`Self::cancel_and_wait`] before releasing its parent SSH transport.
pub struct Transfer {
    cancel: watch::Sender<bool>,
    task: Option<tokio::task::JoinHandle<Result<(), Error>>>,
    terminal: watch::Receiver<Option<TransferEvent>>,
    progress: watch::Receiver<Option<TransferEvent>>,
}

impl Transfer {
    pub(super) fn spawn(
        session: Arc<SftpSession>,
        sources: Sources,
        events: mpsc::Sender<TransferEvent>,
    ) -> Self {
        let (cancel, mut cancelled) = watch::channel(false);
        let (terminal_tx, terminal) = watch::channel(None);
        let (progress_tx, progress) = watch::channel(None);
        let task = tokio::spawn(async move {
            let mut reporter = Reporter::new(events.clone(), progress_tx);
            let result = match sources {
                Sources::File(Direction::Upload { local, remote }) => {
                    upload(&session, &local, &remote, &mut cancelled, &mut reporter).await
                }
                Sources::File(Direction::Download { remote, local }) => {
                    download(&session, &remote, &local, &mut cancelled, &mut reporter).await
                }
                Sources::Selection(directions) => {
                    copy_many(&session, directions, &mut cancelled, &mut reporter).await
                }
            };
            let event = match &result {
                Ok(()) => TransferEvent::Completed,
                Err(Error::Cancelled) => TransferEvent::Cancelled,
                Err(error) => TransferEvent::Failed(error.to_string()),
            };
            let _ = terminal_tx.send(Some(event.clone()));
            let _ = tokio::time::timeout(Duration::from_secs(1), events.send(event)).await;
            result
        });
        Self {
            cancel,
            task: Some(task),
            terminal,
            progress,
        }
    }

    /// Requests cancellation; the terminal event is sent after preserving copied data.
    pub fn cancel(&self) {
        let _ = self.cancel.send(true);
    }

    /// Returns the retained completion, cancellation, or failure state, when available.
    /// This remains available even if a slow UI misses the bounded event delivery.
    pub fn terminal_event(&self) -> Option<TransferEvent> {
        self.terminal.borrow().clone()
    }

    /// Returns the latest progress even when the bounded event queue was full.
    pub fn progress_event(&self) -> Option<TransferEvent> {
        self.progress.borrow().clone()
    }

    /// Cancels and waits for file-handle closure and preservation of copied data.
    /// Keep the parent SSH transport alive until this method returns.
    /// Successful closure of a cancelled or already completed transfer returns `Ok(())`.
    ///
    /// # Errors
    /// Returns transfer, cleanup, or task failures. After 35 seconds, aborts the task
    /// and reports that temporary remote data may remain.
    pub async fn cancel_and_wait(mut self) -> Result<(), Error> {
        self.cancel();
        let Some(mut task) = self.task.take() else {
            return Ok(());
        };
        match tokio::time::timeout(Duration::from_secs(35), &mut task).await {
            Ok(Ok(Ok(()) | Err(Error::Cancelled))) => Ok(()),
            Ok(Ok(Err(error))) => Err(error),
            Ok(Err(_)) => Err(Error::Cleanup),
            Err(_) => {
                task.abort();
                let _ = task.await;
                Err(Error::CleanupTimeout)
            }
        }
    }
}

impl Drop for Transfer {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub(super) async fn copy<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    source: &mut R,
    destination: &mut W,
    cancel: &mut watch::Receiver<bool>,
    progress: &mut Reporter,
) -> Result<(), Error> {
    let mut buffer = [0_u8; 32 * 1024];
    loop {
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn shutdown_deadline_aborts_task_and_reports_possible_partial_data() {
        struct Dropped(Arc<AtomicBool>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let cleanup = Dropped(dropped.clone());
        let task = tokio::spawn(async move {
            let _cleanup = cleanup;
            std::future::pending::<Result<(), Error>>().await
        });
        tokio::task::yield_now().await;
        let (cancel, _cancelled) = watch::channel(false);
        let (_terminal, terminal) = watch::channel(None);
        let (_progress, progress) = watch::channel(None);
        let transfer = Transfer {
            cancel,
            task: Some(task),
            terminal,
            progress,
        };
        let error = transfer.cancel_and_wait().await.unwrap_err();
        assert!(matches!(error, Error::CleanupTimeout));
        assert!(
            error
                .to_string()
                .contains("temporary .latch file may remain")
        );
        assert!(dropped.load(Ordering::SeqCst));
    }
}
