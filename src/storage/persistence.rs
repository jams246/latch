use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};

use uuid::Uuid;

use super::Configuration;

pub(super) fn read(path: &Path) -> io::Result<Configuration> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Configuration::default());
        }
        Err(error) => return Err(error),
    };
    serde_json::from_slice(&bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Connection configuration is invalid; the original file was preserved",
        )
    })
}

pub(super) fn write(path: &Path, configuration: &Configuration) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(configuration).map_err(io::Error::other)?;
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    replace_file(path, &temporary, &bytes)
}

fn replace_file(path: &Path, temporary: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut created = false;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary)?;
        created = true;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        // A same-directory rename preserves the old configuration if replacement fails.
        fs::rename(temporary, path)
    })();
    if result.is_err() && created {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_create_failure_preserves_unowned_temporary_and_existing_configuration() {
        let store = crate::storage::tests_persistence::store("temporary-collision");
        let path = store.path;
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let temporary = path.with_extension("existing.tmp");
        fs::write(&path, b"original configuration").unwrap();
        fs::write(&temporary, b"unrelated existing file").unwrap();
        let error = replace_file(&path, &temporary, b"new configuration").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&temporary).unwrap(), b"unrelated existing file");
        assert_eq!(fs::read(path).unwrap(), b"original configuration");
    }
}
