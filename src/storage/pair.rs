use std::{fs, io, path::Path};

use uuid::Uuid;

use super::files;

pub(super) const FILES: [&str; 2] = ["latch.db", "key.dpapi"];
const SIDECARS: [&str; 3] = ["latch.db-journal", "latch.db-wal", "latch.db-shm"];

pub(super) fn present(directory: &Path) -> io::Result<bool> {
    for name in FILES.into_iter().chain(SIDECARS) {
        if files::exists(&directory.join(name))? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn require(directory: &Path) -> io::Result<()> {
    if !fs::symlink_metadata(directory)?.is_dir() {
        return Err(incomplete());
    }
    for name in FILES {
        let metadata = fs::symlink_metadata(directory.join(name)).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                incomplete()
            } else {
                error
            }
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(incomplete());
        }
    }
    Ok(())
}

pub(super) fn publish(source: &Path, destination: &Path) -> io::Result<()> {
    if present(destination)? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Encrypted settings already exist; no files were overwritten",
        ));
    }
    for name in FILES {
        let target = destination.join(name);
        files::copy_new(&source.join(name), &target)?;
    }
    Ok(())
}

pub(super) fn sync(path: &Path) -> io::Result<()> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?
        .sync_all()
}

pub(super) fn cleanup(directory: &Path) -> io::Result<()> {
    for name in FILES {
        fs::remove_file(directory.join(name))?;
    }
    for name in SIDECARS {
        let path = directory.join(name);
        if files::exists(&path)? {
            fs::remove_file(path)?;
        }
    }
    remove_empty(directory)
}

pub(super) fn remove_empty(directory: &Path) -> io::Result<()> {
    match fs::remove_dir(directory) {
        Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(()),
        result => result,
    }
}

pub(super) fn archive(source: &Path, destination: &Path) -> io::Result<()> {
    let token = Uuid::new_v4();
    for name in SIDECARS.into_iter().chain(FILES) {
        let path = source.join(name);
        if !files::exists(&path)? {
            continue;
        }
        if fs::symlink_metadata(&path)?.is_dir() {
            return Err(incomplete());
        }
        let target = destination.join(format!("{name}.archive-{token}"));
        if files::exists(&target)? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Recovery archive already exists",
            ));
        }
        fs::rename(path, target)?;
    }
    Ok(())
}

pub(super) fn leftovers(directory: &Path) -> io::Result<bool> {
    for entry in fs::read_dir(directory)? {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("latch-data.stage-")
            || name.starts_with("latch-data.archive-")
            || FILES
                .into_iter()
                .chain(SIDECARS)
                .any(|prefix| name.starts_with(&format!("{prefix}.archive-")))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn incomplete() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Encrypted profile is incomplete; import a JSON backup to recover",
    )
}
