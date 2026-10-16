//! SFTP directory browsing and cancellable transfers over an authenticated SSH tab.

mod browser;
mod local_source;
mod operations;
mod transfer;
mod transfer_conflicts;
mod transfer_destination;
mod transfer_files;
mod transfer_interactive;
mod transfer_io;
mod transfer_plan;
mod transfer_progress;
mod transfer_publish;
mod transfer_trees;
mod tree_limits;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_browse;
#[cfg(test)]
mod tests_cancelled;
#[cfg(test)]
mod tests_conflicts;
#[cfg(test)]
mod tests_file_replace;
#[cfg(test)]
mod tests_filesystem;
#[cfg(test)]
mod tests_lifecycle;
#[cfg(test)]
mod tests_merge;
#[cfg(test)]
mod tests_operations;
#[cfg(test)]
mod tests_pause;
#[cfg(test)]
mod tests_progress;
#[cfg(test)]
mod tests_progress_totals;
#[cfg(test)]
mod tests_races;
#[cfg(test)]
mod tests_security;
#[cfg(test)]
pub(crate) mod tests_server;
#[cfg(test)]
mod tests_trees;

use std::{io, path::PathBuf};

pub use browser::Browser;
pub use transfer::Transfer;
pub use transfer_conflicts::{Conflict, ConflictChoice};

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

/// Transfer source and destination. Interactive transfers can request replacement.
#[derive(Clone, Debug)]
pub enum Direction {
    /// Copy a local regular file or directory tree to a new remote path.
    Upload {
        /// Local source file.
        local: PathBuf,
        /// New remote destination file.
        remote: String,
    },
    /// Copy a remote regular file or directory tree to a new local path.
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
    /// Current filename, cumulative bytes copied, and operation length when known.
    Progress {
        /// Source filename currently being copied.
        name: String,
        /// Bytes copied across the complete operation so far.
        completed: u64,
        /// Total source length across the complete operation, when supplied.
        total: Option<u64>,
    },
    /// The completed file was published at its destination.
    Completed,
    /// Cancellation completed and already copied data was retained.
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
    /// A cancelled transfer retained partial data outside its intended destination.
    #[error("Transfer cancelled. Partial data remains at {path}. {reason}")]
    PartialRetained {
        /// Path containing the preserved partial file.
        path: String,
        /// Reason the partial file could not use its intended name.
        reason: String,
    },
    /// A failed replacement retained the original destination at a backup path.
    #[error("Destination replacement failed. Original data remains at {path}. {reason}")]
    ReplacementRetained {
        /// Path containing the preserved original destination.
        path: String,
        /// Reason replacement or restoration could not complete.
        reason: String,
    },
    /// A destination file already exists.
    #[error("Existing destination was preserved. Choose a new filename.")]
    DestinationExists,
    /// The source, destination, or entry name is not a valid path.
    #[error("Choose a valid source path and a valid destination name.")]
    InvalidPath,
    /// A copy source is a symbolic link or a special file.
    #[error("Links and special files cannot be copied. No link target was followed.")]
    UnsupportedCopySource,
    /// A directory tree exceeds the bounded planning or traversal budget.
    #[error("The directory tree is too large or too deep. Select a smaller folder.")]
    TreeLimit,
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
