use std::{sync::Arc, time::Duration};

use russh_sftp::client::SftpSession;
use tokio::sync::{mpsc, oneshot};

use super::{Direction, Directory, Entry, Error, Transfer, TransferEvent};
use crate::ssh::Command;

/// A reusable SFTP channel sharing an already authenticated terminal transport.
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
        let directory = match self.session.read_dir(path.clone()).await {
            Ok(directory) => directory,
            Err(error) => {
                // Closing the SFTP channel also releases a directory handle on read failure.
                let _ = self.session.close().await;
                return Err(error.into());
            }
        };
        let mut entries: Vec<_> = directory
            .map(|entry| Entry {
                name: entry.file_name(),
                path: entry.path(),
                is_dir: entry.file_type().is_dir(),
                size: entry.metadata().size,
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

    /// Spawns a transfer on the current Tokio runtime.
    /// Keep its returned control alive; dropping it requests cancellation and cleanup.
    /// Await [`Transfer::cancel_and_wait`] before closing the parent SSH transport.
    pub fn transfer(&self, direction: Direction, events: mpsc::Sender<TransferEvent>) -> Transfer {
        Transfer::spawn(self.session.clone(), direction, events)
    }
}
