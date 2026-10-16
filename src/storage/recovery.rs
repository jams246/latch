use std::{
    fs, io,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use super::{Configuration, ImportSnapshot, Store, files, locking, pair, persistence, portable};

pub(super) const MARKER: &str = "latch-recovery.pending";
const GUARD: &[u8] = b"LATCH-RECOVERY-V1\n";

impl Store {
    /// Creates a fresh encrypted profile from a validated JSON backup.
    /// Unreadable current data is archived unchanged before publication.
    ///
    /// # Errors
    /// Failed or interrupted recovery preserves data and blocks legacy reimport.
    pub fn recover_portable(directory: &Path, snapshot: ImportSnapshot) -> io::Result<Self> {
        let lock = locking::acquire(directory)?;
        let configuration = snapshot.into_configuration();
        let stage = stage(directory, &configuration)?;
        let marker = directory.join(MARKER);
        write_guard(&marker)?;
        pair::archive(directory, directory)?;
        let old = directory.join(portable::LEGACY_DIRECTORY);
        if files::exists(&old)? && fs::symlink_metadata(&old)?.is_dir() {
            pair::archive(&old, directory)?;
            pair::remove_empty(&old)?;
        }
        archive_stages(directory, &stage)?;
        publish(&stage, directory, &configuration)?;
        let mut store = Self::load(directory.join(portable::DATABASE))?;
        store._lock = Some(lock);
        store.warnings = portable::legacy_warnings(directory, None);
        Ok(store)
    }
}

fn archive_stages(directory: &Path, current: &Path) -> io::Result<()> {
    let stages = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    for path in stages {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path == current
            || !(name.starts_with("latch-data.stage-") || name.starts_with("latch-data.archive-"))
        {
            continue;
        }
        if !fs::symlink_metadata(&path)?.is_dir() {
            continue;
        }
        pair::archive(&path, directory)?;
        pair::remove_empty(&path)?;
    }
    Ok(())
}

pub(super) fn publish(
    source: &Path,
    directory: &Path,
    configuration: &Configuration,
) -> io::Result<()> {
    write_guard(&directory.join(MARKER))?;
    pair::publish(source, directory)?;
    let restored = persistence::read(&directory.join(portable::DATABASE))?;
    if &restored != configuration {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Encrypted profile verification failed; existing data was preserved",
        ));
    }
    pair::cleanup(source)?;
    fs::remove_file(directory.join(MARKER))
}

fn write_guard(path: &Path) -> io::Result<()> {
    if !files::exists(path)? {
        return files::create_new(path, GUARD);
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || files::read_limited(path, GUARD.len())? != GUARD
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Recovery marker is invalid; the original file was preserved",
        ));
    }
    Ok(())
}

pub(super) fn stage(directory: &Path, configuration: &Configuration) -> io::Result<PathBuf> {
    let stage = directory.join(format!("latch-data.stage-{}", Uuid::new_v4()));
    fs::create_dir(&stage)?;
    let path = stage.join(portable::DATABASE);
    persistence::write(&path, configuration)?;
    let restored = persistence::read(&path)?;
    super::validate(&restored)?;
    if &restored != configuration {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Encrypted profile verification failed; existing data was preserved",
        ));
    }
    for path in [&path, &stage.join("key.dpapi")] {
        pair::sync(path)?;
    }
    Ok(stage)
}

pub(super) fn interrupted() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Profile recovery was interrupted; import a JSON backup to finish recovery",
    )
}
