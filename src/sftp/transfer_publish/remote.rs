use super::Controls;
use russh_sftp::{client::SftpSession, protocol::FileAttributes};
use uuid::Uuid;

use super::super::{
    Error,
    operations::{child, read_directory},
    transfer::wait_running,
};

#[derive(Eq, PartialEq)]
struct Stamp {
    size: Option<u64>,
    modified: Option<u32>,
    permissions: Option<u32>,
    uid: Option<u32>,
    gid: Option<u32>,
}

impl From<FileAttributes> for Stamp {
    fn from(attributes: FileAttributes) -> Self {
        Self {
            size: attributes.size,
            modified: attributes.mtime,
            permissions: attributes.permissions,
            uid: attributes.uid,
            gid: attributes.gid,
        }
    }
}

pub(in crate::sftp) struct Snapshot(Vec<(String, Stamp)>);

pub(super) async fn snapshot(
    session: &SftpSession,
    root: &str,
    mut controls: Option<Controls<'_>>,
) -> Result<Snapshot, Error> {
    let mut pending = vec![String::new()];
    let mut entries = Vec::new();
    while let Some(relative) = pending.pop() {
        if let Some((cancel, paused)) = controls.as_mut() {
            wait_running(cancel, paused).await?;
        }
        let path = format!("{root}{relative}");
        let metadata = session.symlink_metadata(&path).await?;
        if metadata.file_type().is_dir() {
            for entry in read_directory(session, &path).await? {
                if let Some((cancel, paused)) = controls.as_mut() {
                    wait_running(cancel, paused).await?;
                }
                pending.push(child(&relative, &entry.file_name())?);
            }
        }
        entries.push((relative, metadata.into()));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(Snapshot(entries))
}

pub(super) async fn publish(
    session: &SftpSession,
    temporary: &str,
    destination: &str,
    expected: &Snapshot,
    controls: Controls<'_>,
) -> Result<(), Error> {
    let (cancel, paused) = controls;
    if snapshot(session, destination, Some((&mut *cancel, &mut *paused)))
        .await?
        .0
        != expected.0
    {
        return Err(Error::DestinationExists);
    }
    wait_running(cancel, paused).await?;
    let parent = destination.rsplit_once('/').ok_or(Error::InvalidPath)?.0;
    let backup = child(parent, &format!(".latch-{}.backup", Uuid::new_v4()))?;
    session.rename(destination, &backup).await?;
    let result = async {
        if snapshot(session, &backup, None).await?.0 != expected.0 {
            return Err(Error::DestinationExists);
        }
        // Standard SFTP rename rejects an existing destination instead of replacing it.
        session.rename(temporary, destination).await?;
        Ok(())
    }
    .await;
    if let Err(error) = result {
        session
            .rename(&backup, destination)
            .await
            .map_err(|reason| retained(&backup, reason))?;
        return Err(error);
    }
    remove(session, &backup)
        .await
        .map_err(|reason| retained(&backup, reason))
}

fn retained(path: &str, reason: impl std::fmt::Display) -> Error {
    Error::ReplacementRetained {
        path: path.to_owned(),
        reason: reason.to_string(),
    }
}

pub(super) async fn remove(session: &SftpSession, path: &str) -> Result<(), Error> {
    let mut pending = vec![(path.to_owned(), false)];
    while let Some((path, visited)) = pending.pop() {
        let metadata = session.symlink_metadata(&path).await?;
        if !metadata.file_type().is_dir() {
            session.remove_file(path).await?;
        } else if visited {
            session.remove_dir(path).await?;
        } else {
            let entries = read_directory(session, &path).await?;
            pending.push((path.clone(), true));
            for entry in entries {
                pending.push((child(&path, &entry.file_name())?, false));
            }
        }
    }
    Ok(())
}
