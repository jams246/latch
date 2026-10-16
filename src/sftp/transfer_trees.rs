use russh_sftp::client::SftpSession;
use tokio::sync::watch;

use super::{
    Direction, Error,
    transfer::wait_running,
    transfer_destination::{create_directory, ensure_new, local_is_link},
    transfer_files::{download, upload},
    transfer_plan::{plan, total_size},
    transfer_progress::Reporter,
};

pub(super) async fn copy_many(
    session: &SftpSession,
    directions: Vec<Direction>,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
    progress: &mut Reporter,
) -> Result<(), Error> {
    let nodes = plan(session, directions, cancel, paused).await?;
    let mut pending: Vec<_> = nodes.iter().collect();
    while let Some(node) = pending.pop() {
        wait_running(cancel, paused).await?;
        ensure_new(session, &node.direction).await?;
        if let Some(children) = &node.children {
            pending.extend(children);
        }
    }
    progress.total(total_size(&nodes));
    let mut pending = nodes;
    pending.reverse();
    while let Some(node) = pending.pop() {
        wait_running(cancel, paused).await?;
        if let Some(mut children) = node.children {
            create_directory(session, &node.direction).await?;
            children.reverse();
            pending.extend(children);
        } else {
            copy_file(session, &node.direction, cancel, paused, progress, None).await?;
        }
    }
    Ok(())
}

pub(super) async fn copy_file(
    session: &SftpSession,
    direction: &Direction,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
    progress: &mut Reporter,
    replacement: Option<&super::transfer_publish::Replacement>,
) -> Result<(), Error> {
    match direction {
        Direction::Upload { local, remote } => {
            let metadata = tokio::fs::symlink_metadata(local).await?;
            if !metadata.is_file() || local_is_link(&metadata) {
                return Err(Error::UnsupportedCopySource);
            }
            upload(
                session,
                local,
                remote,
                cancel,
                paused,
                progress,
                replacement,
            )
            .await
        }
        Direction::Download { remote, local } => {
            if !session
                .symlink_metadata(remote)
                .await?
                .file_type()
                .is_file()
            {
                return Err(Error::UnsupportedCopySource);
            }
            download(
                session,
                remote,
                local,
                cancel,
                paused,
                progress,
                replacement,
            )
            .await
        }
    }
}
