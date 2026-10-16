use russh_sftp::{
    client::{SftpSession, fs::ReadDir},
    protocol::StatusCode,
};

use super::{Browser, Error};

impl Browser {
    /// Renames an entry within its existing directory without replacing another entry.
    ///
    /// # Errors
    /// Returns invalid-name, existing-destination, permissions, or transport failures.
    pub async fn rename(&self, path: &str, name: &str) -> Result<(), Error> {
        validate_path(path)?;
        validate_name(name)?;
        let parent = path.rsplit_once('/').ok_or(Error::InvalidPath)?.0;
        let destination = child(parent, name)?;
        if path == destination {
            return Ok(());
        }
        ensure_new(&self.session, &destination).await?;
        self.session.rename(path, destination).await?;
        Ok(())
    }

    /// Creates a new directory without replacing an existing entry.
    ///
    /// # Errors
    /// Returns invalid-name, existing-destination, permissions, or transport failures.
    pub async fn create_directory(&self, parent: &str, name: &str) -> Result<(), Error> {
        validate_name(name)?;
        let parent = self.session.canonicalize(parent).await?;
        let path = child(&parent, name)?;
        ensure_new(&self.session, &path).await?;
        self.session.create_dir(path).await?;
        Ok(())
    }

    /// Deletes entries recursively. Symbolic links are removed without following them.
    ///
    /// # Errors
    /// Returns invalid-path, permissions, or transport failures. Earlier entries may
    /// already have been deleted when a later entry fails.
    pub async fn delete(&self, paths: &[String]) -> Result<(), Error> {
        for path in paths {
            validate_path(path)?;
        }
        let mut pending: Vec<_> = paths.iter().cloned().map(|path| (path, false)).collect();
        while let Some((path, visited)) = pending.pop() {
            let metadata = self.session.symlink_metadata(&path).await?;
            if !metadata.file_type().is_dir() {
                self.session.remove_file(path).await?;
            } else if visited {
                self.session.remove_dir(path).await?;
            } else {
                let entries = read_directory(&self.session, &path).await?;
                pending.push((path.clone(), true));
                for entry in entries {
                    pending.push((child(&path, &entry.file_name())?, false));
                }
            }
        }
        Ok(())
    }
}

pub(super) async fn read_directory(session: &SftpSession, path: &str) -> Result<ReadDir, Error> {
    match session.read_dir(path).await {
        Ok(directory) => Ok(directory),
        Err(error) => {
            // Closing the SFTP channel also releases a directory handle on read failure.
            let _ = session.close().await;
            Err(error.into())
        }
    }
}

pub(super) fn validate_name(name: &str) -> Result<(), Error> {
    if name.is_empty() || matches!(name, "." | "..") || name.contains(['/', '\\', '\0']) {
        return Err(Error::InvalidPath);
    }
    Ok(())
}

pub(super) fn validate_path(path: &str) -> Result<(), Error> {
    if !path.starts_with('/') || path == "/" || path.contains('\0') {
        return Err(Error::InvalidPath);
    }
    for component in path[1..].split('/') {
        validate_name(component)?;
    }
    Ok(())
}

pub(super) fn child(parent: &str, name: &str) -> Result<String, Error> {
    validate_name(name)?;
    Ok(format!("{}/{name}", parent.trim_end_matches('/')))
}

pub(super) async fn ensure_new(session: &SftpSession, path: &str) -> Result<(), Error> {
    match session.symlink_metadata(path).await {
        Ok(_) => Err(Error::DestinationExists),
        Err(russh_sftp::client::error::Error::Status(status))
            if status.status_code == StatusCode::NoSuchFile =>
        {
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}
