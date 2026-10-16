use std::{sync::Arc, time::Duration};

use russh_sftp::client::SftpSession;
use tokio::sync::{mpsc, oneshot};

use super::{Direction, Directory, Entry, Error, Transfer, TransferEvent};
use super::{operations::read_directory, transfer::Sources};
use crate::ssh::Command;

/// A reusable SFTP channel sharing an already authenticated SSH transport.
#[derive(Clone)]
pub struct Browser {
    pub(super) session: Arc<SftpSession>,
}

impl Browser {
    /// Opens an SFTP channel without authenticating again or increasing shell usage.
    ///
    /// # Errors
    /// Returns disconnected, timeout, and SFTP initialization failures.
    pub async fn connect(commands: &mpsc::Sender<Command>) -> Result<Self, Error> {
        let (reply, receiver) = oneshot::channel();
        commands
            .send(Command::OpenSftp(reply))
            .await
            .map_err(|_| Error::NotConnected)?;
        let stream = tokio::time::timeout(Duration::from_secs(25), receiver)
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::NotConnected)?
            .map_err(|_| Error::NotConnected)?;
        let session = tokio::time::timeout(Duration::from_secs(15), SftpSession::new(stream))
            .await
            .map_err(|_| Error::Timeout)??;
        Ok(Self {
            session: Arc::new(session),
        })
    }

    /// Lists a canonical remote directory with directories before files.
    ///
    /// # Errors
    /// Returns remote lookup, permissions, and transport failures.
    /// A directory-read failure closes this SFTP channel to release its handle;
    /// open a new browser before retrying.
    pub async fn list(&self, path: &str) -> Result<Directory, Error> {
        let path = self.session.canonicalize(path).await?;
        let directory = read_directory(&self.session, &path).await?;
        let mut entries: Vec<_> = directory
            .map(|entry| {
                let metadata = entry.metadata();
                let file_type = metadata.file_type();
                let is_dir = file_type.is_dir();
                Entry {
                    name: entry.file_name(),
                    path: entry.path(),
                    is_dir,
                    is_link: file_type.is_symlink(),
                    size: if is_dir { None } else { metadata.size },
                    modified: metadata.mtime.map(u64::from),
                }
            })
            .collect();
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(Directory { path, entries })
    }

    /// Resolves a symbolic link and returns its canonical directory target.
    /// A regular file target returns `None`.
    ///
    /// # Errors
    /// Returns lookup, broken-link, permissions, and transport failures.
    pub async fn resolve_link(&self, path: &str) -> Result<Option<String>, Error> {
        let path = self.session.canonicalize(path).await?;
        let metadata = self.session.metadata(path.clone()).await?;
        Ok(metadata.file_type().is_dir().then_some(path))
    }

    /// Spawns a transfer on the current Tokio runtime.
    /// Keep its returned control alive; dropping it cancels and retains copied data.
    /// Await [`Transfer::cancel_and_wait`] before closing the parent SSH transport.
    pub fn transfer(&self, direction: Direction, events: mpsc::Sender<TransferEvent>) -> Transfer {
        Transfer::spawn(self.session.clone(), Sources::File(direction), events)
    }

    /// Copies selected files and directory trees sequentially with shared cancellation.
    /// Existing destinations are preserved; links and special files are rejected.
    /// Completed files remain if a later item fails; cancellation also retains partial files.
    pub fn transfer_many(
        &self,
        directions: Vec<Direction>,
        events: mpsc::Sender<TransferEvent>,
    ) -> Transfer {
        Transfer::spawn(self.session.clone(), Sources::Selection(directions), events)
    }

    /// Copies one item, requesting a decision when its destination exists.
    /// An explicitly opened remote link resolves to its canonical source first.
    pub fn transfer_interactive(
        &self,
        direction: Direction,
        events: mpsc::Sender<TransferEvent>,
    ) -> Transfer {
        Transfer::spawn(
            self.session.clone(),
            Sources::InteractiveFile(direction),
            events,
        )
    }

    /// Copies selected files and folders, requesting decisions for destination conflicts.
    /// Links and special source files are rejected before any destination is changed.
    pub fn transfer_many_interactive(
        &self,
        directions: Vec<Direction>,
        events: mpsc::Sender<TransferEvent>,
    ) -> Transfer {
        Transfer::spawn(
            self.session.clone(),
            Sources::Interactive(directions),
            events,
        )
    }
}
