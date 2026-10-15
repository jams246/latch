mod agent;
mod auth;
mod host;
mod session;

use std::path::PathBuf;

use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroizing;

pub use session::{SessionControl, run, spawn};

pub type Secret = Zeroizing<String>;
pub type SftpStream = russh::ChannelStream<russh::client::Msg>;

#[derive(Clone, Debug)]
pub enum AuthMethod {
    Password,
    KeyFile(PathBuf),
    Agent,
    KeyboardInteractive,
}

#[derive(Clone, Debug)]
pub struct ConnectionConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: AuthMethod,
    pub known_hosts: PathBuf,
    pub cols: u32,
    pub rows: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Disconnected,
    Connecting,
    Authenticating,
    Connected,
    Failed,
}

pub enum Command {
    Input(Vec<u8>),
    Resize { cols: u32, rows: u32 },
    Disconnect,
    OpenSftp(oneshot::Sender<Result<SftpStream, Error>>),
}

#[derive(Debug)]
pub enum Event {
    Status(State),
    Output(Vec<u8>),
    Error(String),
    Exit(u32),
}

pub struct ChallengeField {
    pub label: String,
    pub echo: bool,
}

pub enum SecretKind {
    Password,
    KeyPassphrase { path: PathBuf },
}

// Secret replies use dedicated one-shot channels and never enter general app events.
pub enum Prompt {
    HostKey {
        host: String,
        port: u16,
        algorithm: String,
        fingerprint: String,
        reply: oneshot::Sender<bool>,
    },
    Secret {
        kind: SecretKind,
        error: Option<String>,
        reply: oneshot::Sender<Option<Secret>>,
    },
    KeyboardInteractive {
        name: String,
        instructions: String,
        fields: Vec<ChallengeField>,
        reply: oneshot::Sender<Option<Vec<Secret>>>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Connection cancelled")]
    Cancelled,
    #[error("Connection or authentication timed out. Check the server and try again.")]
    Timeout,
    #[error("Invalid hostname, port, username, or terminal dimensions")]
    InvalidConfig,
    #[error("SSH transport failed. Check the hostname, port, network, and server settings.")]
    Transport,
    #[error(
        "The server host key changed. Connection blocked. Verify it out of band before updating known hosts."
    )]
    HostKeyChanged,
    #[error("Cannot read or save trusted host keys. Check the known-hosts file permissions.")]
    KnownHosts,
    #[error("SSH host certificates are unsupported. Use a server with a trusted public host key.")]
    HostCertificate,
    #[error("Authentication rejected. Check the username and selected authentication method.")]
    Authentication,
    #[error(
        "Cannot read or unlock the selected private key. Check its path, format, and passphrase."
    )]
    PrivateKey,
    #[error("SSH agent unavailable or no agent key was accepted")]
    Agent,
    #[error("The server sent an unsupported keyboard-interactive challenge")]
    Challenge,
    #[error("The server refused the remote PTY, shell, or SFTP subsystem")]
    ChannelRejected,
}

impl From<russh::Error> for Error {
    fn from(_: russh::Error) -> Self {
        Self::Transport
    }
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
mod tests_auth;
#[cfg(test)]
mod tests_lifecycle;
#[cfg(test)]
mod tests_timeouts;
