use std::{io, path::PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Authentication preference. Credentials are never part of connection metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthMethod {
    /// Ask for a password after verifying the server's host key.
    Password,
    /// Use an existing key file without copying its contents into configuration.
    KeyFile {
        /// Path to the existing private key.
        path: PathBuf,
    },
    /// Ask an existing SSH agent to sign authentication requests.
    Agent,
    /// Display the server's supported ephemeral authentication challenges.
    KeyboardInteractive,
}

/// The three supported connection-picker orders.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    /// Show the complete alphabetical list.
    #[default]
    Name,
    /// Show the complete list by descending successful-shell count.
    MostUsed,
    /// Show the top three, then the complete alphabetical list with duplicates.
    FrequentAndName,
}

/// Saved non-secret connection metadata and its usage count.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    /// Stable identity retained when the connection is renamed.
    pub id: String,
    /// Name displayed in the picker.
    pub name: String,
    /// Hostname or IP address, without a port or URL scheme.
    pub host: String,
    /// SSH port.
    pub port: u16,
    /// SSH account name.
    pub username: String,
    /// Preferred authentication method, containing no secrets.
    pub auth: AuthMethod,
    /// Number of successfully started, user-initiated terminal shells.
    pub successful_shells: u64,
}

impl Connection {
    /// Creates metadata with a new stable ID, port 22, and no recorded uses.
    pub fn new(name: String, host: String, username: String, auth: AuthMethod) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            host,
            port: 22,
            username,
            auth,
            successful_shells: 0,
        }
    }

    pub(super) fn validate(&self) -> io::Result<()> {
        for value in [&self.id, &self.name, &self.host, &self.username] {
            if value.trim().is_empty() || value.chars().any(char::is_control) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Connection identity, name, host, and username must be nonempty text",
                ));
            }
        }
        if self.port == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SSH port must be between 1 and 65535",
            ));
        }
        if let AuthMethod::KeyFile { path } = &self.auth
            && path.as_os_str().is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SSH key path must not be empty",
            ));
        }
        Ok(())
    }
}
