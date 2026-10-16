use std::{
    fs, io,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use super::{Configuration, ImportSnapshot, Store, files, locking, persistence, portable};

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
        let stage = stage(directory, &snapshot.into_configuration())?;
        let marker = directory.join(MARKER);
        write_guard(&marker)?;
        let active = directory.join(portable::DATA_DIRECTORY);
        if files::exists(&active)? {
            let archive = directory.join(format!("latch-data.archive-{}", Uuid::new_v4()));
            fs::rename(&active, archive)?;
        }
        fs::rename(stage, &active)?;
        portable::require_pair(&active)?;
        let mut store = Self::load(active.join(portable::DATABASE))?;
        fs::remove_file(marker)?;
        store._lock = Some(lock);
        store.warnings = portable::legacy_warnings(directory, None);
        Ok(store)
    }
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
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?
            .sync_all()?;
    }
    Ok(stage)
}

pub(super) fn interrupted() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Profile recovery was interrupted; import a JSON backup to finish recovery",
    )
}
