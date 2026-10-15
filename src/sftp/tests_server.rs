use std::collections::{HashMap, HashSet};

use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode,
};

pub(crate) async fn serve(stream: russh::ChannelStream<russh::server::Msg>) {
    serve_faulty(stream, false, false).await;
}

pub(super) async fn serve_faulty(
    stream: russh::ChannelStream<russh::server::Msg>,
    stall_close: bool,
    deny_remove: bool,
) {
    let files = HashMap::from([
        ("/hello.txt".into(), b"Synthetic SFTP content\n".to_vec()),
        (
            "/folder/nested.txt".into(),
            b"Nested synthetic content\n".to_vec(),
        ),
        ("/large.bin".into(), vec![0x5a; 16 * 1024 * 1024]),
    ]);
    russh_sftp::server::run(
        stream,
        Filesystem {
            files,
            directories: HashSet::new(),
            stall_close,
            deny_remove,
        },
    )
    .await;
}

struct Filesystem {
    files: HashMap<String, Vec<u8>>,
    directories: HashSet<String>,
    stall_close: bool,
    deny_remove: bool,
}

fn normalized(path: &str) -> String {
    let mut components = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => (),
            ".." => {
                components.pop();
            }
            name => components.push(name),
        }
    }
    format!("/{}", components.join("/"))
}

fn status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: "en".into(),
    }
}

fn attributes(size: u64, directory: bool) -> FileAttributes {
    FileAttributes {
        size: Some(size),
        permissions: Some(if directory { 0o040755 } else { 0o100644 }),
        ..Default::default()
    }
}

impl Filesystem {
    fn attrs(&self, path: &str) -> Result<FileAttributes, StatusCode> {
        let path = normalized(path);
        if path == "/" || path == "/folder" {
            Ok(attributes(0, true))
        } else {
            self.files
                .get(&path)
                .map(|file| attributes(file.len() as u64, false))
                .ok_or(StatusCode::NoSuchFile)
        }
    }
}

impl russh_sftp::server::Handler for Filesystem {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let path = normalized(&path);
        Ok(Name {
            id,
            files: vec![File::new(&path, self.attrs(&path)?)],
        })
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let path = normalized(&path);
        if !self.attrs(&path)?.is_dir() {
            return Err(StatusCode::NoSuchFile);
        }
        self.directories.remove(&path);
        Ok(Handle { id, handle: path })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        if !self.directories.insert(handle.clone()) {
            return Err(StatusCode::Eof);
        }
        let prefix = if handle == "/" {
            "/".into()
        } else {
            format!("{handle}/")
        };
        let mut files = Vec::new();
        if handle == "/" {
            files.push(File::new("folder", attributes(0, true)));
        }
        for (path, content) in &self.files {
            if let Some(name) = path.strip_prefix(&prefix)
                && !name.contains('/')
            {
                files.push(File::new(name, attributes(content.len() as u64, false)));
            }
        }
        Ok(Name { id, files })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        Ok(Attrs {
            id,
            attrs: self.attrs(&path)?,
        })
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        Ok(Attrs {
            id,
            attrs: self.attrs(&path)?,
        })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        Ok(Attrs {
            id,
            attrs: self.attrs(&handle)?,
        })
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let path = normalized(&filename);
        if flags.contains(OpenFlags::EXCLUDE) && self.files.contains_key(&path) {
            return Err(StatusCode::Failure);
        }
        if flags.contains(OpenFlags::CREATE) {
            self.files.entry(path.clone()).or_default();
        }
        if !self.files.contains_key(&path) {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(Handle { id, handle: path })
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        length: u32,
    ) -> Result<Data, Self::Error> {
        if handle == "/large.bin" {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let file = self.files.get(&handle).ok_or(StatusCode::NoSuchFile)?;
        let offset = usize::try_from(offset).map_err(|_| StatusCode::Eof)?;
        if offset >= file.len() {
            return Err(StatusCode::Eof);
        }
        let end = (offset + length.min(64 * 1024) as usize).min(file.len());
        Ok(Data {
            id,
            data: file[offset..end].to_vec(),
        })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let file = self.files.get_mut(&handle).ok_or(StatusCode::NoSuchFile)?;
        let offset = usize::try_from(offset).map_err(|_| StatusCode::Failure)?;
        let end = offset
            .checked_add(data.len())
            .filter(|end| *end <= 64 * 1024 * 1024)
            .ok_or(StatusCode::Failure)?;
        file.resize(file.len().max(end), 0);
        file[offset..end].copy_from_slice(&data);
        Ok(status(id))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        if self.stall_close {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        self.directories.remove(&handle);
        Ok(status(id))
    }

    async fn rename(&mut self, id: u32, old: String, new: String) -> Result<Status, Self::Error> {
        let (old, new) = (normalized(&old), normalized(&new));
        if self.files.contains_key(&new) {
            return Err(StatusCode::Failure);
        }
        let content = self.files.remove(&old).ok_or(StatusCode::NoSuchFile)?;
        self.files.insert(new, content);
        Ok(status(id))
    }

    async fn remove(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        if self.deny_remove {
            return Err(StatusCode::PermissionDenied);
        }
        self.files
            .remove(&normalized(&path))
            .ok_or(StatusCode::NoSuchFile)?;
        Ok(status(id))
    }
}
