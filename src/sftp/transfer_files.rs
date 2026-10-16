use std::path::{Path, PathBuf};

use russh_sftp::{client::SftpSession, protocol::OpenFlags};
use tokio::{
    fs::{self, File, OpenOptions},
    sync::{mpsc, watch},
};
use uuid::Uuid;

use super::{
    Error, TransferEvent,
    operations::ensure_new,
    transfer::{check_cancel, copy},
};

struct LocalTemporary {
    path: PathBuf,
    armed: bool,
}

impl Drop for LocalTemporary {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
pub(super) async fn upload(
    session: &SftpSession,
    local: &Path,
    remote: &str,
    cancel: &mut watch::Receiver<bool>,
    events: &mpsc::Sender<TransferEvent>,
) -> Result<(), Error> {
    validate_remote(remote)?;
    check_cancel(cancel)?;
    let mut source = File::open(local).await?;
    let metadata = source.metadata().await?;
    if !metadata.is_file() {
        return Err(Error::InvalidPath);
    }
    ensure_new(session, remote).await?;
    let temporary = remote_temporary(remote);
    let mut destination = session
        .open_with_flags(
            &temporary,
            OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
        )
        .await?;
    let result = copy(
        &mut source,
        &mut destination,
        Some(metadata.len()),
        cancel,
        events,
    )
    .await;
    let closed = destination.close().await.map_err(Error::RemoteClose);
    let result = async {
        closed?;
        result?;
        check_cancel(cancel)?;
        // Standard SFTP rename rejects an existing destination instead of replacing it.
        session.rename(&temporary, remote).await?;
        Ok(())
    }
    .await;
    if result.is_err() && session.remove_file(&temporary).await.is_err() {
        return Err(Error::Cleanup);
    }
    result
}

pub(super) async fn download(
    session: &SftpSession,
    remote: &str,
    local: &Path,
    cancel: &mut watch::Receiver<bool>,
    events: &mpsc::Sender<TransferEvent>,
) -> Result<(), Error> {
    validate_remote(remote)?;
    check_cancel(cancel)?;
    match fs::symlink_metadata(local).await {
        Ok(_) => return Err(Error::DestinationExists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    if local.file_name().is_none() {
        return Err(Error::InvalidPath);
    }
    let mut source = session.open(remote).await?;
    let total = source.metadata().await?.size;
    let mut temporary = LocalTemporary {
        path: local.with_file_name(format!(".latch-{}.part", Uuid::new_v4())),
        armed: false,
    };
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary.path)
        .await?;
    temporary.armed = true;
    let result = copy(&mut source, &mut destination, total, cancel, events).await;
    let closed = source.close().await.map_err(Error::RemoteClose);
    let result = async {
        closed?;
        result?;
        destination.sync_all().await?;
        drop(destination);
        check_cancel(cancel)?;
        // An atomic hard link never overwrites a destination created during the transfer.
        fs::hard_link(&temporary.path, local)
            .await
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    Error::DestinationExists
                } else {
                    Error::Local(error)
                }
            })?;
        Ok(())
    }
    .await;
    if fs::remove_file(&temporary.path).await.is_err() {
        return Err(Error::Cleanup);
    }
    temporary.armed = false;
    result
}

fn validate_remote(path: &str) -> Result<(), Error> {
    if path.contains('\0') || matches!(path.rsplit('/').next(), Some("" | "." | "..") | None) {
        return Err(Error::InvalidPath);
    }
    Ok(())
}

fn remote_temporary(path: &str) -> String {
    let name = format!(".latch-{}.part", Uuid::new_v4());
    match path.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{name}"),
        None => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn aborted_download_task_removes_its_local_temporary_file() {
        let path = crate::sftp::tests::directory().join(".latch-aborted.part");
        let temporary = LocalTemporary {
            path: path.clone(),
            armed: true,
        };
        let (ready, receiver) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _temporary = temporary;
            let _file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&_temporary.path)
                .await
                .unwrap();
            ready.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        receiver.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(!path.exists());
    }
}
