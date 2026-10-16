use std::path::{Component, Path, PathBuf};

use russh_sftp::client::SftpSession;
use tokio::{fs, sync::watch};

use super::{
    Direction, Error,
    operations::{child, ensure_new, read_directory, validate_path},
    transfer::check_cancel,
    transfer_files::{download, upload},
    transfer_progress::Reporter,
};

enum Item {
    UploadDirectory(String),
    DownloadDirectory(PathBuf),
    File(Direction),
}

struct Plan {
    items: Vec<Item>,
    total: Option<u64>,
}

pub(super) async fn copy_many(
    session: &SftpSession,
    directions: Vec<Direction>,
    cancel: &mut watch::Receiver<bool>,
    progress: &mut Reporter,
) -> Result<(), Error> {
    let plan = plan(session, directions, cancel).await?;
    progress.total(plan.total);
    for item in plan.items {
        check_cancel(cancel)?;
        match item {
            Item::UploadDirectory(path) => session.create_dir(path).await?,
            Item::DownloadDirectory(path) => {
                fs::create_dir(path).await.map_err(destination_error)?
            }
            Item::File(Direction::Upload { local, remote }) => {
                if !fs::symlink_metadata(&local).await?.is_file() {
                    return Err(Error::UnsupportedCopySource);
                }
                upload(session, &local, &remote, cancel, progress).await?;
            }
            Item::File(Direction::Download { remote, local }) => {
                if !session
                    .symlink_metadata(&remote)
                    .await?
                    .file_type()
                    .is_file()
                {
                    return Err(Error::UnsupportedCopySource);
                }
                download(session, &remote, &local, cancel, progress).await?;
            }
        }
    }
    Ok(())
}

async fn plan(
    session: &SftpSession,
    mut pending: Vec<Direction>,
    cancel: &watch::Receiver<bool>,
) -> Result<Plan, Error> {
    let mut plan = Vec::new();
    let mut total = Some(0_u64);
    pending.reverse();
    while let Some(direction) = pending.pop() {
        check_cancel(cancel)?;
        match direction {
            Direction::Upload { local, remote } => {
                validate_path(&remote)?;
                let metadata = fs::symlink_metadata(&local).await?;
                let kind = metadata.file_type();
                ensure_new(session, &remote).await?;
                if kind.is_file() {
                    total = total.and_then(|total| total.checked_add(metadata.len()));
                    plan.push(Item::File(Direction::Upload { local, remote }));
                } else if kind.is_dir() {
                    let mut entries = fs::read_dir(&local).await?;
                    while let Some(entry) = entries.next_entry().await? {
                        check_cancel(cancel)?;
                        let name = entry.file_name();
                        let name = name.to_str().ok_or(Error::InvalidPath)?;
                        pending.push(Direction::Upload {
                            local: entry.path(),
                            remote: child(&remote, name)?,
                        });
                    }
                    plan.push(Item::UploadDirectory(remote));
                } else {
                    return Err(Error::UnsupportedCopySource);
                }
            }
            Direction::Download { remote, local } => {
                validate_path(&remote)?;
                let metadata = session.symlink_metadata(&remote).await?;
                let kind = metadata.file_type();
                ensure_local_new(&local).await?;
                if kind.is_file() {
                    total = total
                        .zip(metadata.size)
                        .and_then(|(total, size)| total.checked_add(size));
                    plan.push(Item::File(Direction::Download { remote, local }));
                } else if kind.is_dir() {
                    for entry in read_directory(session, &remote).await? {
                        check_cancel(cancel)?;
                        let name = entry.file_name();
                        validate_local_name(&name)?;
                        pending.push(Direction::Download {
                            remote: child(&remote, &name)?,
                            local: local.join(name),
                        });
                    }
                    plan.push(Item::DownloadDirectory(local));
                } else {
                    return Err(Error::UnsupportedCopySource);
                }
            }
        }
    }
    Ok(Plan { items: plan, total })
}

async fn ensure_local_new(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path).await {
        Ok(_) => Err(Error::DestinationExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validate_local_name(name: &str) -> Result<(), Error> {
    super::operations::validate_name(name)?;
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(Error::InvalidPath);
    }
    #[cfg(windows)]
    {
        let stem = name
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end_matches(' ')
            .to_ascii_uppercase();
        let numbered = stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"));
        if name.contains([':', '<', '>', '"', '|', '?', '*'])
            || name.ends_with([' ', '.'])
            || name.chars().any(|character| character.is_control())
            || matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
            || numbered.is_some_and(|number| {
                matches!(
                    number,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        {
            return Err(Error::InvalidPath);
        }
    }
    Ok(())
}

fn destination_error(error: std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        Error::DestinationExists
    } else {
        Error::Local(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_child_names_cannot_escape_local_destination() {
        for name in [
            "",
            ".",
            "..",
            "/absolute",
            "../escape",
            "a/b",
            "a\\b",
            "nul\0",
        ] {
            assert!(validate_local_name(name).is_err(), "Accepted {name:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn server_child_names_reject_windows_devices_and_aliases() {
        for name in [
            "CON",
            "nul.txt",
            "COM1.txt",
            "LPT9",
            "COM¹",
            "NUL .txt",
            "CONOUT$",
            "file:stream",
            "trailing.",
            "trailing ",
        ] {
            assert!(validate_local_name(name).is_err(), "Accepted {name:?}");
        }
    }
}
