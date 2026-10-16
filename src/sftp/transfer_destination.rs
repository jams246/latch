use std::path::Path;

use russh_sftp::{client::SftpSession, protocol::StatusCode};
use tokio::{fs, sync::watch};

use super::{Direction, Error, operations::child, transfer::wait_running, transfer_plan::Node};

pub(super) async fn existing(
    session: &SftpSession,
    direction: &Direction,
) -> Result<Option<bool>, Error> {
    match direction {
        Direction::Upload { remote, .. } => match session.symlink_metadata(remote).await {
            Ok(metadata) => Ok(Some(metadata.file_type().is_dir())),
            Err(russh_sftp::client::error::Error::Status(status))
                if status.status_code == StatusCode::NoSuchFile =>
            {
                Ok(None)
            }
            Err(error) => Err(error.into()),
        },
        Direction::Download { local, .. } => match fs::symlink_metadata(local).await {
            Ok(metadata) => Ok(Some(metadata.is_dir() && !local_is_link(&metadata))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        },
    }
}

pub(super) fn local_is_link(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
}

pub(super) async fn ensure_new(session: &SftpSession, direction: &Direction) -> Result<(), Error> {
    if existing(session, direction).await?.is_some() {
        Err(Error::DestinationExists)
    } else {
        Ok(())
    }
}

pub(super) async fn create_directory(
    session: &SftpSession,
    direction: &Direction,
) -> Result<(), Error> {
    match direction {
        Direction::Upload { remote, .. } => session.create_dir(remote).await.map_err(Error::Remote),
        Direction::Download { local, .. } => fs::create_dir(local).await.map_err(Error::Local),
    }
}

pub(super) fn name(direction: &Direction) -> Result<String, Error> {
    match direction {
        Direction::Upload { remote, .. } => remote
            .rsplit('/')
            .next()
            .map(str::to_owned)
            .ok_or(Error::InvalidPath),
        Direction::Download { local, .. } => local
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .ok_or(Error::InvalidPath),
    }
}

pub(super) fn renamed(direction: &Direction, name: &str) -> Result<Direction, Error> {
    Ok(match direction {
        Direction::Upload { local, remote } => Direction::Upload {
            local: local.clone(),
            remote: child(remote.rsplit_once('/').ok_or(Error::InvalidPath)?.0, name)?,
        },
        Direction::Download { remote, local } => Direction::Download {
            remote: remote.clone(),
            local: local.with_file_name(name),
        },
    })
}

pub(super) fn rebase_children(children: &mut [Node], parent: &Direction) -> Result<(), Error> {
    for node in children {
        let name = name(&node.direction)?;
        node.direction = match (&node.direction, parent) {
            (Direction::Upload { local, .. }, Direction::Upload { remote, .. }) => {
                Direction::Upload {
                    local: local.clone(),
                    remote: child(remote, &name)?,
                }
            }
            (Direction::Download { remote, .. }, Direction::Download { local, .. }) => {
                Direction::Download {
                    remote: remote.clone(),
                    local: local.join(name),
                }
            }
            _ => return Err(Error::InvalidPath),
        };
    }
    Ok(())
}

pub(super) async fn duplicate(
    session: &SftpSession,
    direction: &Direction,
    is_dir: bool,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
) -> Result<Direction, Error> {
    let name = name(direction)?;
    for number in 1_u64.. {
        wait_running(cancel, paused).await?;
        let destination = renamed(direction, &numbered(&name, is_dir, number))?;
        if existing(session, &destination).await?.is_none() {
            return Ok(destination);
        }
    }
    Err(Error::DestinationExists)
}

fn numbered(name: &str, is_dir: bool, number: u64) -> String {
    if !is_dir
        && let Some(extension) = Path::new(name)
            .extension()
            .and_then(|extension| extension.to_str())
    {
        let stem = &name[..name.len() - extension.len() - 1];
        format!("{stem} ({number}).{extension}")
    } else {
        format!("{name} ({number})")
    }
}

#[cfg(test)]
mod tests {
    use super::numbered;

    #[test]
    fn duplicate_numbers_precede_last_file_extension_and_follow_complete_folder_name() {
        for (name, folder, expected) in [
            ("File.txt", false, "File (2).txt"),
            ("archive.tar.gz", false, "archive.tar (2).gz"),
            ("File", false, "File (2)"),
            (".env", false, ".env (2)"),
            ("Folder.ext", true, "Folder.ext (2)"),
        ] {
            assert_eq!(numbered(name, folder, 2), expected);
        }
    }
}
