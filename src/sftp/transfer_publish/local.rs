use std::{
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use tokio::fs;
use uuid::Uuid;

use super::{
    super::{Error, transfer::wait_running},
    Controls,
};

#[derive(Eq, PartialEq)]
struct Stamp {
    size: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    directory: bool,
    link: bool,
    readonly: bool,
}

pub(in crate::sftp) struct Snapshot(Vec<(PathBuf, Stamp)>);

pub(super) async fn snapshot(
    root: &Path,
    mut controls: Option<Controls<'_>>,
) -> Result<Snapshot, Error> {
    let mut pending = vec![PathBuf::new()];
    let mut entries = Vec::new();
    while let Some(relative) = pending.pop() {
        if let Some((cancel, paused)) = controls.as_mut() {
            wait_running(cancel, paused).await?;
        }
        let path = if relative.as_os_str().is_empty() {
            root.to_owned()
        } else {
            root.join(&relative)
        };
        let metadata = fs::symlink_metadata(&path).await?;
        let link = is_link(&metadata);
        if metadata.is_dir() && !link {
            let mut children = fs::read_dir(&path).await?;
            while let Some(child) = children.next_entry().await? {
                if let Some((cancel, paused)) = controls.as_mut() {
                    wait_running(cancel, paused).await?;
                }
                pending.push(relative.join(child.file_name()));
            }
        }
        entries.push((
            relative,
            Stamp {
                size: metadata.len(),
                modified: metadata.modified().ok(),
                created: metadata.created().ok(),
                directory: metadata.is_dir(),
                link,
                readonly: metadata.permissions().readonly(),
            },
        ));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(Snapshot(entries))
}

pub(super) async fn publish(
    temporary: &Path,
    destination: &Path,
    expected: &Snapshot,
    controls: Controls<'_>,
) -> Result<(), Error> {
    let (cancel, paused) = controls;
    if snapshot(destination, Some((&mut *cancel, &mut *paused)))
        .await?
        .0
        != expected.0
    {
        return Err(Error::DestinationExists);
    }
    wait_running(cancel, paused).await?;
    let backup = destination.with_file_name(format!(".latch-{}.backup", Uuid::new_v4()));
    rename_new(destination, &backup).await?;
    let result = async {
        if snapshot(&backup, None).await?.0 != expected.0 {
            return Err(Error::DestinationExists);
        }
        rename_new(temporary, destination).await
    }
    .await;
    if let Err(error) = result {
        rename_new(&backup, destination)
            .await
            .map_err(|reason| retained(&backup, reason))?;
        return Err(error);
    }
    remove(&backup)
        .await
        .map_err(|reason| retained(&backup, reason))
}

fn retained(path: &Path, reason: impl std::fmt::Display) -> Error {
    Error::ReplacementRetained {
        path: path.to_string_lossy().into_owned(),
        reason: reason.to_string(),
    }
}

pub(super) async fn remove(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path).await?;
    #[cfg(windows)]
    if is_link(&metadata) {
        use std::os::windows::fs::MetadataExt;
        use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
        return if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY.0 != 0 {
            fs::remove_dir(path).await
        } else {
            fs::remove_file(path).await
        };
    }
    if metadata.is_dir() && !is_link(&metadata) {
        fs::remove_dir_all(path).await
    } else if metadata.is_dir() {
        fs::remove_dir(path).await
    } else {
        fs::remove_file(path).await
    }
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
    }
    #[cfg(not(windows))]
    metadata.file_type().is_symlink()
}

async fn rename_new(source: &Path, destination: &Path) -> Result<(), Error> {
    let (source, destination) = (source.to_owned(), destination.to_owned());
    tokio::task::spawn_blocking(move || rename_without_replace(&source, &destination))
        .await
        .map_err(|_| Error::Cleanup)?
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                Error::DestinationExists
            } else {
                Error::Local(error)
            }
        })
}

#[cfg(windows)]
fn rename_without_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{Win32::Storage::FileSystem::MoveFileW, core::PCWSTR};
    let wide = |path: &Path| -> io::Result<Vec<u16>> {
        let mut value: Vec<_> = path.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Path contains NUL",
            ));
        }
        value.push(0);
        Ok(value)
    };
    let source = wide(source)?;
    let destination = wide(destination)?;
    // MoveFileW rejects concurrently created destinations instead of replacing them.
    unsafe { MoveFileW(PCWSTR(source.as_ptr()), PCWSTR(destination.as_ptr())) }
        .map_err(|error| io::Error::from_raw_os_error(error.code().0 & 0xffff))
}

#[cfg(not(windows))]
// shortcut: folder replacement is unavailable, add platform atomic no-replace rename to support it.
fn rename_without_replace(source: &Path, destination: &Path) -> io::Result<()> {
    if !std::fs::symlink_metadata(source)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Replacing folders without concurrent overwrites is unavailable on this platform",
        ));
    }
    std::fs::hard_link(source, destination)?;
    if let Err(error) = std::fs::remove_file(source) {
        let _ = std::fs::remove_file(destination);
        return Err(error);
    }
    Ok(())
}
