use std::io;

use zeroize::Zeroizing;

use super::{AuthMethod, Configuration, Connection, Store};

#[cfg(test)]
mod tests;

impl Store {
    /// Returns a saved password only for the connection's current account and destination.
    pub fn saved_password(&self, connection: &Connection) -> Option<&Zeroizing<String>> {
        self.connections()
            .iter()
            .find(|saved| saved.id == connection.id && same_account(saved, connection))?;
        self.configuration.passwords.get(&connection.id)
    }

    /// Persists a successfully authenticated password in the encrypted database.
    /// Stale sessions cannot attach credentials to an edited or deleted connection.
    ///
    /// # Errors
    /// Returns database errors without changing the previously saved password.
    pub fn save_password(
        &mut self,
        connection: &Connection,
        password: Zeroizing<String>,
    ) -> io::Result<()> {
        if !matches!(connection.auth, AuthMethod::Password)
            || !self
                .connections()
                .iter()
                .any(|saved| saved.id == connection.id && same_account(saved, connection))
        {
            return Ok(());
        }
        self.update(|configuration| {
            configuration
                .passwords
                .insert(connection.id.clone(), password);
            Ok(())
        })
    }

    /// Removes a saved password without changing connection metadata or live sessions.
    ///
    /// # Errors
    /// Returns database errors without claiming the password was removed.
    pub fn clear_password(&mut self, id: &str) -> io::Result<()> {
        self.update(|configuration| {
            configuration.passwords.remove(id);
            Ok(())
        })
    }
}

pub(super) fn same_account(left: &Connection, right: &Connection) -> bool {
    left.host == right.host
        && left.port == right.port
        && left.username == right.username
        && left.auth == right.auth
}

pub(super) fn validate(configuration: &Configuration) -> io::Result<()> {
    if configuration.passwords.keys().any(|id| {
        !configuration
            .connections
            .iter()
            .any(|connection| connection.id == *id && connection.auth == AuthMethod::Password)
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Saved password has no matching password connection",
        ));
    }
    Ok(())
}
