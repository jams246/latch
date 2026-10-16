use std::{collections::HashSet, sync::Arc};

use russh_sftp::protocol::{Attrs, File, FileAttributes, Handle, Name, Status, StatusCode};
use tokio::sync::mpsc;

use super::{
    Browser, Direction, Error, TransferEvent,
    tests::{directory, finished},
};

struct DirectoryServer {
    name: &'static str,
    canonical: &'static str,
    listed: HashSet<String>,
}

impl russh_sftp::server::Handler for DirectoryServer {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn realpath(&mut self, id: u32, _: String) -> Result<Name, Self::Error> {
        Ok(Name {
            id,
            files: vec![File::dummy(self.canonical)],
        })
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        Ok(Handle { id, handle: path })
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        if !self.listed.insert(handle) {
            return Err(StatusCode::Eof);
        }
        Ok(Name {
            id,
            files: vec![File::new(self.name, directory_attributes())],
        })
    }

    async fn lstat(&mut self, id: u32, _: String) -> Result<Attrs, Self::Error> {
        Ok(Attrs {
            id,
            attrs: directory_attributes(),
        })
    }

    async fn close(&mut self, id: u32, _: String) -> Result<Status, Self::Error> {
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: String::new(),
        })
    }
}

fn directory_attributes() -> FileAttributes {
    FileAttributes {
        permissions: Some(0o040755),
        ..Default::default()
    }
}

async fn browser(name: &'static str, canonical: &'static str) -> Browser {
    let (client, server) = tokio::io::duplex(1024);
    tokio::spawn(russh_sftp::server::run(
        server,
        DirectoryServer {
            name,
            canonical,
            listed: HashSet::new(),
        },
    ));
    Browser {
        session: Arc::new(russh_sftp::client::SftpSession::new(client).await.unwrap()),
    }
}

#[tokio::test]
async fn browsing_rejects_server_names_that_escape_the_listed_directory() {
    for name in [
        "../secret",
        "/absolute",
        "folder/secret",
        "folder\\secret",
        "nul\0name",
    ] {
        let browser = browser(name, "/").await;
        assert!(
            matches!(browser.list("/").await, Err(Error::InvalidPath)),
            "Accepted {name:?}"
        );
    }
}

#[tokio::test]
async fn browsing_rejects_invalid_server_canonical_paths() {
    for path in ["relative", "/../escape", "/folder/..", "/nul\0name"] {
        let browser = browser("child", path).await;
        assert!(
            matches!(browser.list("/").await, Err(Error::InvalidPath)),
            "Accepted {path:?}"
        );
        browser.session.close().await.unwrap();
    }
}

#[tokio::test]
async fn endless_remote_tree_fails_preflight_without_creating_local_data() {
    let browser = browser("child", "/").await;
    let root = directory();
    let (events, mut incoming) = mpsc::channel(32);
    let _transfer = browser.transfer_many(
        vec![Direction::Download {
            remote: "/tree".into(),
            local: root.join("tree"),
        }],
        events,
    );
    assert!(
        matches!(finished(&mut incoming).await, TransferEvent::Failed(message) if message.contains("too large or too deep"))
    );
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    browser.session.close().await.unwrap();
}
