use std::path::{Path, PathBuf};

use russh_sftp::client::SftpSession;
use tokio::sync::watch;

use super::{Direction, Error};

mod local;
mod remote;

type Controls<'a> = (&'a mut watch::Receiver<bool>, &'a mut watch::Receiver<bool>);

pub(super) enum Replacement {
    Local {
        path: PathBuf,
        snapshot: local::Snapshot,
    },
    Remote {
        path: String,
        snapshot: remote::Snapshot,
    },
}

pub(super) async fn capture(
    session: &SftpSession,
    direction: &Direction,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
) -> Result<Replacement, Error> {
    match direction {
        Direction::Upload { remote: path, .. } => Ok(Replacement::Remote {
            path: path.clone(),
            snapshot: remote::snapshot(session, path, Some((cancel, paused))).await?,
        }),
        Direction::Download { local: path, .. } => Ok(Replacement::Local {
            path: path.clone(),
            snapshot: local::snapshot(path, Some((cancel, paused))).await?,
        }),
    }
}

pub(super) async fn publish(
    session: &SftpSession,
    staged: &Direction,
    replacement: &Replacement,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
) -> Result<(), Error> {
    match staged {
        Direction::Upload { remote, .. } => {
            let Replacement::Remote { path, .. } = replacement else {
                return Err(Error::InvalidPath);
            };
            replacement
                .publish_remote(session, remote, path, cancel, paused)
                .await
        }
        Direction::Download { local, .. } => {
            let Replacement::Local { path, .. } = replacement else {
                return Err(Error::InvalidPath);
            };
            replacement.publish_local(local, path, cancel, paused).await
        }
    }
}

pub(super) async fn remove(session: &SftpSession, direction: &Direction) -> Result<(), Error> {
    match direction {
        Direction::Upload { remote, .. } => remote::remove(session, remote).await,
        Direction::Download { local, .. } => local::remove(local).await.map_err(Error::Local),
    }
}

impl Replacement {
    pub(super) async fn publish_remote(
        &self,
        session: &SftpSession,
        temporary: &str,
        destination: &str,
        cancel: &mut watch::Receiver<bool>,
        paused: &mut watch::Receiver<bool>,
    ) -> Result<(), Error> {
        match self {
            Self::Remote { path, snapshot } if path == destination && temporary != destination => {
                remote::publish(session, temporary, path, snapshot, (cancel, paused)).await
            }
            _ => Err(Error::InvalidPath),
        }
    }

    pub(super) async fn publish_local(
        &self,
        temporary: &Path,
        destination: &Path,
        cancel: &mut watch::Receiver<bool>,
        paused: &mut watch::Receiver<bool>,
    ) -> Result<(), Error> {
        match self {
            Self::Local { path, snapshot } if path == destination && temporary != destination => {
                local::publish(temporary, path, snapshot, (cancel, paused)).await
            }
            _ => Err(Error::InvalidPath),
        }
    }
}
