use std::{sync::Arc, time::Duration};

use russh_sftp::client::SftpSession;
use tokio::sync::{mpsc, watch};

use super::{
    Conflict, ConflictChoice, Direction, Error, TransferEvent,
    transfer_conflicts::Resolver,
    transfer_files::{download, upload},
    transfer_progress::Reporter,
    transfer_trees::copy_many,
};

pub(super) use super::transfer_io::{check_cancel, copy, wait_running};

pub(super) enum Sources {
    File(Direction),
    Selection(Vec<Direction>),
    Interactive(Vec<Direction>),
    InteractiveFile(Direction),
}

/// Cooperative transfer cancellation, including file closure when its owning tab closes.
/// Await [`Self::cancel_and_wait`] before releasing its parent SSH transport.
pub struct Transfer {
    cancel: watch::Sender<bool>,
    paused: watch::Sender<bool>,
    task: Option<tokio::task::JoinHandle<Result<(), Error>>>,
    terminal: watch::Receiver<Option<TransferEvent>>,
    progress: watch::Receiver<Option<TransferEvent>>,
    conflict: watch::Sender<Option<Conflict>>,
    resolutions: mpsc::UnboundedSender<(u64, ConflictChoice)>,
}

impl Transfer {
    pub(super) fn spawn(
        session: Arc<SftpSession>,
        sources: Sources,
        events: mpsc::Sender<TransferEvent>,
    ) -> Self {
        let (cancel, mut cancelled) = watch::channel(false);
        let (paused, mut pause_requested) = watch::channel(false);
        let (terminal_tx, terminal) = watch::channel(None);
        let (progress_tx, progress) = watch::channel(None);
        let (mut resolver, conflict, resolutions) = Resolver::channel();
        let task = tokio::spawn(async move {
            let mut reporter = Reporter::new(events.clone(), progress_tx);
            let result = match sources {
                Sources::File(Direction::Upload { local, remote }) => {
                    upload(
                        &session,
                        &local,
                        &remote,
                        &mut cancelled,
                        &mut pause_requested,
                        &mut reporter,
                        None,
                    )
                    .await
                }
                Sources::File(Direction::Download { remote, local }) => {
                    download(
                        &session,
                        &remote,
                        &local,
                        &mut cancelled,
                        &mut pause_requested,
                        &mut reporter,
                        None,
                    )
                    .await
                }
                Sources::Selection(directions) => {
                    copy_many(
                        &session,
                        directions,
                        &mut cancelled,
                        &mut pause_requested,
                        &mut reporter,
                    )
                    .await
                }
                Sources::Interactive(directions) => {
                    super::transfer_interactive::copy_many(
                        &session,
                        directions,
                        &mut cancelled,
                        &mut pause_requested,
                        &mut reporter,
                        &mut resolver,
                    )
                    .await
                }
                Sources::InteractiveFile(direction) => {
                    async {
                        let direction = match direction {
                            Direction::Download { remote, local } => Direction::Download {
                                remote: session.canonicalize(remote).await?,
                                local,
                            },
                            upload => upload,
                        };
                        super::transfer_interactive::copy_many(
                            &session,
                            vec![direction],
                            &mut cancelled,
                            &mut pause_requested,
                            &mut reporter,
                            &mut resolver,
                        )
                        .await
                    }
                    .await
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
            paused,
            task: Some(task),
            terminal,
            progress,
            conflict,
            resolutions,
        }
    }

    /// Requests cancellation; the terminal event is sent after preserving copied data.
    pub fn cancel(&self) {
        let _ = self.cancel.send(true);
    }

    /// Pauses before the next chunk, retaining the current file handles and offsets.
    pub fn pause(&self) {
        self.paused.send_replace(true);
    }

    /// Continues the same transfer from its retained position.
    pub fn resume(&self) {
        self.paused.send_replace(false);
    }

    /// Whether a pause has been requested. An in-flight chunk may still finish.
    pub fn is_paused(&self) -> bool {
        *self.paused.borrow()
    }

    /// Returns the destination conflict currently awaiting a decision.
    pub fn conflict(&self) -> Option<Conflict> {
        if *self.cancel.borrow() {
            None
        } else {
            self.conflict.borrow().clone()
        }
    }

    /// Resolves the matching request once. Stale requests and unavailable merges are rejected.
    pub fn resolve_conflict(&self, id: u64, choice: ConflictChoice) -> bool {
        if *self.cancel.borrow() {
            return false;
        }
        let accepted = self.conflict.send_if_modified(|pending| {
            if pending.as_ref().is_some_and(|conflict| {
                conflict.id == id && (choice != ConflictChoice::Merge || conflict.can_merge)
            }) {
                *pending = None;
                true
            } else {
                false
            }
        });
        accepted && self.resolutions.send((id, choice)).is_ok()
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

#[cfg(test)]
mod tests;
