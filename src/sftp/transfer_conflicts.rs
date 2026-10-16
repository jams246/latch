use tokio::sync::{mpsc, watch};

use super::{Direction, Error, transfer::check_cancel};

/// The action selected for an existing transfer destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictChoice {
    /// Leave the existing destination unchanged and omit this source.
    Skip,
    /// Replace the destination after the source has been copied successfully.
    Replace,
    /// Copy to the first available numbered sibling name.
    Duplicate,
    /// Combine folders, duplicating existing files within them.
    Merge,
}

/// An existing destination awaiting a user decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conflict {
    /// Unique request number within this transfer.
    pub id: u64,
    /// Source path being copied.
    pub source: String,
    /// Existing destination path.
    pub destination: String,
    /// Whether the source is a directory.
    pub is_dir: bool,
    /// Whether both source and destination are regular directories.
    pub can_merge: bool,
}

pub(super) struct Resolver {
    next: u64,
    pending: watch::Sender<Option<Conflict>>,
    replies: mpsc::UnboundedReceiver<(u64, ConflictChoice)>,
}

pub(super) type ConflictChannels = (
    Resolver,
    watch::Sender<Option<Conflict>>,
    mpsc::UnboundedSender<(u64, ConflictChoice)>,
);

impl Resolver {
    pub(super) fn channel() -> ConflictChannels {
        let (pending, _) = watch::channel(None);
        let (responses, replies) = mpsc::unbounded_channel();
        (
            Self {
                next: 0,
                pending: pending.clone(),
                replies,
            },
            pending,
            responses,
        )
    }

    pub(super) async fn choose(
        &mut self,
        direction: &Direction,
        is_dir: bool,
        can_merge: bool,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<ConflictChoice, Error> {
        check_cancel(cancel)?;
        self.next += 1;
        let (source, destination) = match direction {
            Direction::Upload { local, remote } => {
                (local.to_string_lossy().into_owned(), remote.clone())
            }
            Direction::Download { remote, local } => {
                (remote.clone(), local.to_string_lossy().into_owned())
            }
        };
        self.pending.send_replace(Some(Conflict {
            id: self.next,
            source,
            destination,
            is_dir,
            can_merge,
        }));
        let result = loop {
            tokio::select! {
                _ = cancel.changed() => break Err(Error::Cancelled),
                response = self.replies.recv() => match response {
                    Some((id, choice)) if id == self.next && (choice != ConflictChoice::Merge || can_merge) => break Ok(choice),
                    Some(_) => (),
                    None => break Err(Error::Cancelled),
                },
            }
        };
        self.pending.send_replace(None);
        result
    }
}
