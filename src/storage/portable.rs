use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};

use uuid::Uuid;

use super::{Store, persistence, validate};

impl Store {
    /// Loads portable settings, importing an older configuration only once.
    /// Existing portable settings and host trust always take precedence.
    ///
    /// # Errors
    /// Invalid metadata and filesystem failures preserve the original files.
    pub fn load_portable(directory: &Path, roaming_directory: Option<&Path>) -> io::Result<Self> {
        let path = directory.join("latch.json");
        match fs::symlink_metadata(&path) {
            Ok(_) => return Self::load(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        for legacy_directory in [Some(directory), roaming_directory].into_iter().flatten() {
            let Some(configuration) =
                persistence::read_existing(&legacy_directory.join("connections.json"))?
            else {
                continue;
            };
            validate(&configuration)?;
            copy_host_trust(legacy_directory, directory)?;
            let bytes = serde_json::to_vec_pretty(&configuration).map_err(io::Error::other)?;
            create_if_absent(&path, &bytes)?;
            return Self::load(path);
        }
        Self::load(path)
    }
}

fn copy_host_trust(source: &Path, destination: &Path) -> io::Result<()> {
    match fs::symlink_metadata(destination.join("known_hosts")) {
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let bytes = match fs::read(source.join("known_hosts")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    create_if_absent(&destination.join("known_hosts"), &bytes)
}

fn create_if_absent(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    publish_new_file(path, &temporary, bytes)
}

fn publish_new_file(path: &Path, temporary: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = written.and_then(|()| match fs::hard_link(temporary, path) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        result => result,
    });
    let _ = fs::remove_file(temporary);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_destination_wins_and_migration_temporary_is_removed() {
        let path = crate::storage::tests_persistence::path("migration-collision");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"existing configuration").unwrap();

        create_if_absent(&path, b"migrated configuration").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"existing configuration");
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn failed_publication_removes_only_its_temporary_without_creating_destination() {
        let path = crate::storage::tests_persistence::path("migration-publication-failure");
        let directory = path.parent().unwrap();
        fs::create_dir_all(directory).unwrap();
        let temporary = directory.join("migration.tmp");
        let missing_parent = directory.join("missing/latch.json");

        let error = publish_new_file(&missing_parent, &temporary, b"configuration").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(!temporary.exists());
        assert!(!missing_parent.exists());
    }

    #[test]
    fn temporary_collision_preserves_unowned_file() {
        let path = crate::storage::tests_persistence::path("migration-temporary-collision");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let temporary = path.with_extension("existing.tmp");
        fs::write(&temporary, b"unrelated file").unwrap();

        let error = publish_new_file(&path, &temporary, b"configuration").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(temporary).unwrap(), b"unrelated file");
        assert!(!path.exists());
    }
}
