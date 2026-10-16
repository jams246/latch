//! SFTP directory browsing and cancellable transfers over an authenticated SSH tab.

mod browser;
mod transfer;
mod transfer_files;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_browse;
#[cfg(test)]
mod tests_filesystem;
#[cfg(test)]
mod tests_lifecycle;
#[cfg(test)]
mod tests_progress;
#[cfg(test)]
mod tests_races;
#[cfg(test)]
pub(crate) mod tests_server;

use std::{io, path::PathBuf};

pub use browser::Browser;
pub use transfer::Transfer;

/// One non-secret remote directory entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// Filename shown in the browser.
    pub name: String,
    /// Absolute remote path returned from the canonical directory.
    pub path: String,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Whether this entry is a symbolic link.
    pub is_link: bool,
    /// File length when supplied by the server.
    pub size: Option<u64>,
    /// Last modification time in seconds since the Unix epoch, when supplied.
    pub modified: Option<u64>,
}

/// A canonical remote directory and its entries, with directories first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Directory {
    /// Canonical remote directory path.
    pub path: String,
    /// Entries sorted by directory status and filename.
    pub entries: Vec<Entry>,
}

/// Transfer source and destination. Existing destinations are never overwritten.
#[derive(Clone, Debug)]
pub enum Direction {
    /// Copy a local regular file to a new remote path.
    Upload {
        /// Local source file.
        local: PathBuf,
        /// New remote destination file.
        remote: String,
    },
    /// Copy a remote file to a new local path.
    Download {
        /// Remote source file.
        remote: String,
        /// New local destination file.
        local: PathBuf,
    },
}

/// Transfer state delivered without credentials or file contents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransferEvent {
    /// Bytes copied so far, and source length when known.
    Progress {
        /// Bytes copied so far.
        completed: u64,
        /// Source file length, when supplied.
        total: Option<u64>,
    },
    /// The completed file was published at its destination.
    Completed,
    /// Cancellation completed and the temporary destination was removed.
    Cancelled,
    /// The transfer failed, with a displayable diagnostic.
    Failed(String),
}

/// SFTP errors that the interface can display without exposing authentication state.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The parent SSH tab no longer provides an authenticated channel.
    #[error("SFTP channel unavailable. Reconnect and try again.")]
    NotConnected,
    /// Opening the SFTP channel took too long.
    #[error("SFTP timed out. Check the connection and try again.")]
    Timeout,
    /// The user cancelled this transfer.
    #[error("Transfer cancelled")]
    Cancelled,
    /// A destination file already exists.
    #[error("Existing destination was preserved. Choose a new filename.")]
    DestinationExists,
    /// The source or destination is not a valid file path.
    #[error("Choose a regular source file and a valid destination filename.")]
    InvalidPath,
    /// A temporary file could not be removed after a failure.
    #[error(
        "Transfer cleanup failed. A temporary .latch file may remain in the destination folder."
    )]
    Cleanup,
    /// The cleanup task exceeded its bounded shutdown period.
    #[error(
        "Transfer cleanup timed out. A temporary .latch file may remain in the destination folder."
    )]
    CleanupTimeout,
    /// The remote file did not acknowledge closure.
    #[error("The remote transfer file could not be closed: {0}. Reconnect before trying again.")]
    RemoteClose(#[source] io::Error),
    /// A local filesystem operation failed.
    #[error("Local file operation failed: {0}")]
    Local(#[from] io::Error),
    /// The remote server rejected an SFTP operation.
    #[error("Remote file operation failed: {0}")]
    Remote(#[from] russh_sftp::client::error::Error),
}
