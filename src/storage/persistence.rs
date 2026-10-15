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
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        // A same-directory rename preserves the old configuration if replacement fails.
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
