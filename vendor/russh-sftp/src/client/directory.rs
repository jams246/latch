use std::{collections::VecDeque, mem::size_of, sync::Arc, time::Duration};

use super::{
    error::Error,
    fs::{Metadata, ReadDir},
    runtime, RawSftpSession,
};
use crate::protocol::{File, StatusCode};

const MAX_ENTRIES: usize = 100_000;
const MAX_BYTES: usize = 16 * 1024 * 1024;

pub(super) async fn read_dir(session: &RawSftpSession, path: String) -> Result<ReadDir, Error> {
    let parent = Arc::from(path.as_str());
    let handle = session.opendir(path).await?.handle;
    let mut entries = VecDeque::new();
    let mut bytes = 0;
    let result = runtime::timeout(Duration::from_secs(30), async {
        loop {
            match session.readdir(handle.as_str()).await {
                Ok(name) => append(&mut entries, &mut bytes, name.files)?,
                Err(Error::Status(status)) if status.status_code == StatusCode::Eof => {
                    return Ok(())
                }
                Err(error) => return Err(error),
            }
        }
    })
    .await;
    // Release the directory handle even when a peer exceeds a limit or rejects a read.
    let closed = session.close(handle).await;
    result??;
    closed?;
    Ok(ReadDir { parent, entries })
}

fn append(
    entries: &mut VecDeque<(String, Metadata)>,
    bytes: &mut usize,
    files: Vec<File>,
) -> Result<(), Error> {
    if files.is_empty() {
        return Err(Error::UnexpectedBehavior(
            "Empty directory response without EOF".into(),
        ));
    }
    if files.len() > MAX_ENTRIES.saturating_sub(entries.len()) {
        return Err(Error::Limited("directory entry count".into()));
    }
    for file in files {
        let length = size_of::<(String, Metadata)>()
            .saturating_add(file.filename.len())
            .saturating_add(file.attrs.user.as_ref().map_or(0, String::len))
            .saturating_add(file.attrs.group.as_ref().map_or(0, String::len));
        if length > MAX_BYTES.saturating_sub(*bytes) {
            return Err(Error::Limited("directory size".into()));
        }
        *bytes += length;
        entries.push_back((file.filename, file.attrs));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
