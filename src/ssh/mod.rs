//! Independent asynchronous SSH sessions with host trust and ephemeral authentication.

mod agent;
mod auth;
mod error;
mod host;
mod session;

use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroizing;

pub use error::Error;
pub use session::{SessionControl, run, spawn};

/// A short-lived authentication response that clears its owned string on drop.
pub type Secret = Zeroizing<String>;
/// A subsystem stream opened on an already authenticated tab transport.
pub type SftpStream = russh::ChannelStream<russh::client::Msg>;

/// Preferred authentication method; contains no secret material.
#[derive(Clone, Debug)]
pub enum AuthMethod {
    /// Ask for a password only after completing host-key verification.
    Password,
    /// Read an existing private-key file, prompting only if it requires unlocking.
    KeyFile(PathBuf),
    /// Request signatures from the existing system SSH agent without forwarding it.
    Agent,
    /// Display supported server challenges with their own labels and echo settings.
    KeyboardInteractive,
}

/// Non-secret settings for one independently owned terminal connection attempt.
#[derive(Clone, Debug)]
pub struct ConnectionConfig {
    /// Connection label associated with authentication UI.
    pub name: String,
    /// Hostname or IP address without brackets or a port suffix.
    pub host: String,
    /// Remote SSH TCP port.
    pub port: u16,
    /// Remote account receiving authentication requests.
    pub username: String,
    /// Authentication preference for this attempt.
    pub auth: AuthMethod,
    /// Dedicated OpenSSH-compatible file containing trusted public host keys.
    pub known_hosts: PathBuf,
    /// Initial terminal column count; later resize commands replace this value.
    pub cols: u32,
    /// Initial terminal row count; later resize commands replace this value.
    pub rows: u32,
}

/// Connection lifecycle reported to the UI without authentication secrets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// No remote terminal session is active.
    Disconnected,
    /// Establishing the transport and verifying the host identity.
    Connecting,
    /// The verified host is awaiting authentication.
    Authenticating,
    /// Authentication, remote PTY allocation, and shell startup succeeded.
    Connected,
    /// The attempt terminated with a sanitized error.
    Failed,
}

/// Typed requests sent from the UI to one tab's transport task.
pub enum Command {
    /// Raw terminal input bytes, including remote-program prompts inside the terminal.
    Input(Vec<u8>),
    /// Update remote PTY dimensions, including while authentication is pending.
    Resize {
        /// Terminal column count, greater than zero.
        cols: u32,
        /// Terminal row count, greater than zero.
        rows: u32,
    },
    /// Cancel pending authentication or close the connected transport.
    Disconnect,
    /// Open SFTP on this authenticated transport without another login or usage count.
    OpenSftp(oneshot::Sender<Result<SftpStream, Error>>),
}

/// Transport output; login credentials are never carried by this event channel.
#[derive(Debug)]
pub enum Event {
    /// Updated connection lifecycle.
    Status(State),
    /// Raw terminal output for the terminal emulator.
    Output(Vec<u8>),
    /// Actionable text without secret values or server-supplied diagnostic text.
    Error(String),
    /// Exit status reported by the remote shell.
    Exit(u32),
}

/// One server-provided keyboard-interactive field.
pub struct ChallengeField {
    /// Server-provided label, preserved instead of relabeling it as a password.
    pub label: String,
    /// Whether the server explicitly permits visible input for this field.
    pub echo: bool,
}

/// The native masked credential dialog that this attempt currently requires.
pub enum SecretKind {
    /// A remote-account password.
    Password,
    /// A passphrase unlocking the indicated existing private-key file.
    KeyPassphrase {
        /// Existing private-key file that requires unlocking.
        path: PathBuf,
    },
}

// Secret replies use dedicated one-shot channels and never enter general app events.
/// A request for explicit trust or an ephemeral authentication response.
pub enum Prompt {
    /// Unknown public host key; accepting persists the public key before authentication.
    HostKey {
        /// Destination hostname or IP address.
        host: String,
        /// Destination port, included in host-identity matching.
        port: u16,
        /// Presented public-key algorithm.
        algorithm: String,
        /// SHA-256 fingerprint for independent verification.
        fingerprint: String,
        /// Explicit trust decision; false or a dropped sender cancels the attempt.
        reply: oneshot::Sender<bool>,
    },
    /// A password or key passphrase, with no persistent reuse across attempts.
    Secret {
        /// Required credential and associated existing key path, when applicable.
        kind: SecretKind,
        /// Sanitized retry error; a retry always requires a newly entered response.
        error: Option<String>,
        /// A new response or explicit cancellation; dropped senders cancel safely.
        reply: oneshot::Sender<Option<Secret>>,
    },
    /// A clearly identified server challenge with ephemeral, individually labeled fields.
    KeyboardInteractive {
        /// Server-provided challenge name.
        name: String,
        /// Server-provided instructions.
        instructions: String,
        /// Ordered challenge labels and echo settings.
        fields: Vec<ChallengeField>,
        /// Responses in field order, or explicit cancellation.
        reply: oneshot::Sender<Option<Vec<Secret>>>,
    },
}

pub(crate) async fn prompt<T>(
    prompts: &mpsc::Sender<Prompt>,
    request: Prompt,
    response: oneshot::Receiver<T>,
) -> Result<T, Error> {
    prompts.send(request).await.map_err(|_| Error::Cancelled)?;
    tokio::time::timeout(std::time::Duration::from_secs(120), response)
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::Cancelled)
}

#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod fixture_handler;
#[cfg(test)]
mod gui_fixture;
#[cfg(test)]
mod tests_agent;
#[cfg(test)]
mod tests_auth;
#[cfg(test)]
mod tests_lifecycle;
#[cfg(test)]
mod tests_timeouts;
