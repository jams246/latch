use std::path::{Path, PathBuf};

use russh_sftp::{
    client::SftpSession,
    protocol::{OpenFlags, StatusCode},
};
use tokio::{
    fs::{self, File, OpenOptions},
    sync::watch,
};
use uuid::Uuid;

use super::{
    Error,
    operations::ensure_new,
    transfer::{check_cancel, copy, wait_running},
    transfer_progress::Reporter,
    transfer_publish::Replacement,
};

struct LocalTemporary {
    path: PathBuf,
    armed: bool,
    cancel: watch::Receiver<bool>,
}

impl Drop for LocalTemporary {
    fn drop(&mut self) {
        if self.armed && check_cancel(&self.cancel).is_ok() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
pub(super) async fn upload(
    session: &SftpSession,
    local: &Path,
    remote: &str,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
    progress: &mut Reporter,
    replacement: Option<&Replacement>,
) -> Result<(), Error> {
    validate_remote(remote)?;
    wait_running(cancel, paused).await?;
    let mut source = File::open(local).await?;
    let metadata = source.metadata().await?;
    if !metadata.is_file() {
        return Err(Error::InvalidPath);
    }
    if replacement.is_none() {
        ensure_new(session, remote).await?;
    }
    let temporary = remote_temporary(remote);
    let mut destination = session
        .open_with_flags(
            &temporary,
            OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
        )
        .await?;
    progress.file(
        local
            .file_name()
            .ok_or(Error::InvalidPath)?
            .to_string_lossy()
            .into_owned(),
        Some(metadata.len()),
    );
    let result = copy(&mut source, &mut destination, cancel, paused, progress).await;
    let closed = destination.close().await.map_err(Error::RemoteClose);
    if matches!(result, Err(Error::Cancelled)) || check_cancel(cancel).is_err() {
        closed.map_err(|error| retained(&temporary, error))?;
        if replacement.is_some() {
            return Err(retained(&temporary, Error::DestinationExists));
        }
        session
            .rename(&temporary, remote)
            .await
            .map_err(|error| retained(&temporary, error))?;
        return Err(Error::Cancelled);
    }
    let result = async {
        closed?;
        result?;
        if let Some(replacement) = replacement {
            replacement
                .publish_remote(session, &temporary, remote, cancel, paused)
                .await?;
        } else {
            // Standard SFTP rename rejects an existing destination instead of replacing it.
            session.rename(&temporary, remote).await?;
        }
        Ok(())
    }
    .await;
    if matches!(result, Err(Error::Cancelled)) {
        return Err(retained(&temporary, Error::DestinationExists));
    }
    if result.is_err() {
        match session.remove_file(&temporary).await {
            Ok(()) => (),
            Err(russh_sftp::client::error::Error::Status(status))
                if replacement.is_some() && status.status_code == StatusCode::NoSuchFile => {}
            Err(_) if matches!(result, Err(Error::ReplacementRetained { .. })) => return result,
            Err(_) => return Err(Error::Cleanup),
        }
    }
    result
}

pub(super) async fn download(
    session: &SftpSession,
    remote: &str,
    local: &Path,
    cancel: &mut watch::Receiver<bool>,
    paused: &mut watch::Receiver<bool>,
    progress: &mut Reporter,
    replacement: Option<&Replacement>,
) -> Result<(), Error> {
    validate_remote(remote)?;
    wait_running(cancel, paused).await?;
    match fs::symlink_metadata(local).await {
        Ok(_) if replacement.is_none() => return Err(Error::DestinationExists),
        Ok(_) => (),
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
        cancel: cancel.clone(),
    };
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary.path)
        .await?;
    temporary.armed = true;
    progress.file(
        remote.rsplit('/').next().unwrap_or(remote).to_owned(),
        total,
    );
    let result = copy(&mut source, &mut destination, cancel, paused, progress).await;
    let closed = source.close().await.map_err(Error::RemoteClose);
    let synced = destination.sync_all().await;
    drop(destination);
    if matches!(result, Err(Error::Cancelled)) || check_cancel(cancel).is_err() {
        temporary.armed = false;
        let path = temporary.path.to_string_lossy();
        synced.map_err(|error| retained(&path, error))?;
        if replacement.is_some() {
            closed.map_err(|error| retained(&path, error))?;
            return Err(retained(&path, Error::DestinationExists));
        }
        publish_local(&temporary.path, local)
            .await
            .map_err(|error| retained(&path, error))?;
        fs::remove_file(&temporary.path)
            .await
            .map_err(|error| retained(&path, error))?;
        closed.map_err(|error| retained(&local.to_string_lossy(), error))?;
        return Err(Error::Cancelled);
    }
    let result = async {
        closed?;
        result?;
        synced?;
        if let Some(replacement) = replacement {
            replacement
                .publish_local(&temporary.path, local, cancel, paused)
                .await?;
        } else {
            publish_local(&temporary.path, local).await?;
        }
        Ok(())
    }
    .await;
    if matches!(result, Err(Error::Cancelled)) {
        temporary.armed = false;
        return Err(retained(
            &temporary.path.to_string_lossy(),
            Error::DestinationExists,
        ));
    }
    if replacement.is_none() || result.is_err() {
        match fs::remove_file(&temporary.path).await {
            Ok(()) => (),
            Err(error) if replacement.is_some() && error.kind() == std::io::ErrorKind::NotFound => {
            }
            Err(_) if matches!(result, Err(Error::ReplacementRetained { .. })) => return result,
            Err(_) => return Err(Error::Cleanup),
        }
    }
    temporary.armed = false;
    result
}

async fn publish_local(temporary: &Path, local: &Path) -> Result<(), Error> {
    // An atomic hard link never overwrites a destination created during the transfer.
    fs::hard_link(temporary, local).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            Error::DestinationExists
        } else {
            Error::Local(error)
        }
    })
}

fn retained(path: &str, reason: impl std::fmt::Display) -> Error {
    Error::PartialRetained {
        path: path.to_owned(),
        reason: reason.to_string(),
    }
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
mod tests;
