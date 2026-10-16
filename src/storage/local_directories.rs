use std::{io, path::Path};

use super::{Configuration, Store};

#[cfg(test)]
mod tests;

impl Store {
    /// Returns the remembered SFTP local directory for a saved connection.
    pub fn local_directory(&self, id: &str) -> Option<&Path> {
        self.configuration
            .local_directories
            .get(id)
            .map(AsRef::as_ref)
    }

    /// Persists a local directory without changing other application metadata.
    ///
    /// # Errors
    /// Rejects missing connections, invalid paths, and failed database writes.
    pub fn set_local_directory(&mut self, id: &str, path: &Path) -> io::Result<()> {
        if !self
            .connections()
            .iter()
            .any(|connection| connection.id == id)
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Connection was deleted",
            ));
        }
        if self.local_directory(id) == Some(path) {
            return Ok(());
        }
        self.update(|configuration| {
            configuration
                .local_directories
                .insert(id.to_owned(), path.to_path_buf());
            Ok(())
        })
    }
}

pub(super) fn validate(configuration: &Configuration) -> io::Result<()> {
    for (id, path) in &configuration.local_directories {
        if !configuration
            .connections
            .iter()
            .any(|connection| &connection.id == id)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Local directory preferences contain an invalid connection or path",
            ));
        }
        validate_path(path)?;
    }
    Ok(())
}

pub(super) fn validate_path(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty()
        || path
            .to_str()
            .is_none_or(|path| path.chars().any(char::is_control))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Local directory preferences contain an invalid path",
        ));
    }
    Ok(())
}
