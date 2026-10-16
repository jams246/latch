use std::{
    fs::{self, File, OpenOptions},
    io,
    path::Path,
    sync::Arc,
};

pub(super) const LOCK_FILE: &str = "latch.profile.lock";

pub(super) fn acquire(directory: &Path) -> io::Result<Arc<File>> {
    fs::create_dir_all(directory)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(LOCK_FILE))?;
    file.try_lock().map_err(|_| {
        io::Error::new(
            io::ErrorKind::WouldBlock,
            "This Latch profile is already open, or its lock is unavailable",
        )
    })?;
    Ok(Arc::new(file))
}
