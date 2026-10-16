/// Sanitized connection errors that never include credentials or server-supplied text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The tab, dialog, or user cancelled this attempt.
    #[error("Connection cancelled")]
    Cancelled,
    /// A connection, channel, or authentication prompt exceeded its deadline.
    #[error("Connection or authentication timed out. Check the server and try again.")]
    Timeout,
    /// Required metadata or terminal dimensions are invalid.
    #[error("Invalid hostname, port, username, or terminal dimensions")]
    InvalidConfig,
    /// The underlying encrypted transport failed.
    #[error("SSH transport failed. Check the hostname, port, network, and server settings.")]
    Transport,
    /// A stored host identity differs from the server's presented key.
    #[error(
        "The server host key changed. Connection blocked. Verify it out of band before updating known hosts."
    )]
    HostKeyChanged,
    /// Trusted public keys could not be read or persisted safely.
    #[error("Cannot read or save trusted host keys. Check the known-hosts file permissions.")]
    KnownHosts,
    /// Host-certificate authentication is outside the supported public-key trust mechanism.
    #[error("SSH host certificates are unsupported. Use a server with a trusted public host key.")]
    HostCertificate,
    /// The server rejected the selected authentication method.
    #[error("Authentication rejected. Check the username and selected authentication method.")]
    Authentication,
    /// The existing private-key file could not be read or decoded.
    #[error(
        "Cannot read or unlock the selected private key. Check its path, format, and passphrase."
    )]
    PrivateKey,
    /// The existing agent is unavailable or none of its public identities authenticated.
    #[error("SSH agent unavailable or no agent key was accepted")]
    Agent,
    /// A challenge exceeds supported prompt or round limits.
    #[error("The server sent an unsupported keyboard-interactive challenge")]
    Challenge,
    /// The server explicitly rejected a requested terminal or subsystem channel.
    #[error("The server refused the remote PTY, shell, or SFTP subsystem")]
    ChannelRejected,
    /// The server exceeded the buffer allowed before acknowledging channel startup.
    #[error("The server sent too much output before starting the remote channel")]
    ChannelOutputLimit,
}

impl From<russh::Error> for Error {
    fn from(_: russh::Error) -> Self {
        Self::Transport
    }
}
