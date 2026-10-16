use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::{
    client::SftpSession,
    protocol::{Handle, Name, Status},
    server,
};

#[test]
fn directory_rejects_entry_count_overflow() {
    let mut entries = std::iter::repeat_with(|| (String::new(), Metadata::default()))
        .take(MAX_ENTRIES)
        .collect();
    assert!(matches!(
        append(&mut entries, &mut 0, vec![File::dummy("extra")]),
        Err(Error::Limited(_))
    ));
}

#[test]
fn directory_rejects_retained_byte_overflow() {
    let mut bytes = MAX_BYTES;
    assert!(matches!(
        append(&mut VecDeque::new(), &mut bytes, vec![File::dummy("extra")]),
        Err(Error::Limited(_))
    ));
}

#[test]
fn directory_preserves_entries_across_response_batches() {
    let mut entries = VecDeque::new();
    let mut bytes = 0;
    append(&mut entries, &mut bytes, vec![File::dummy("first")]).unwrap();
    append(&mut entries, &mut bytes, vec![File::dummy("second")]).unwrap();
    assert_eq!(
        entries.into_iter().map(|entry| entry.0).collect::<Vec<_>>(),
        ["first", "second"]
    );
}

#[test]
fn directory_skips_many_dot_entries_without_recursive_stack_growth() {
    let entries = std::iter::repeat_with(|| (".".to_owned(), Metadata::default()))
        .take(MAX_ENTRIES)
        .chain(std::iter::once(("file".into(), Metadata::default())))
        .collect();
    let mut directory = ReadDir {
        parent: "/".into(),
        entries,
    };
    assert_eq!(directory.next().unwrap().path(), "/file");
    assert!(directory.next().is_none());
}

enum Response {
    Empty,
    Count,
    Bytes,
    Slow,
}

struct EmptyDirectory(Arc<AtomicBool>, Response);

#[cfg_attr(feature = "async-trait", async_trait::async_trait)]
impl server::Handler for EmptyDirectory {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn opendir(&mut self, id: u32, _: String) -> Result<Handle, Self::Error> {
        Ok(Handle {
            id,
            handle: "directory".into(),
        })
    }

    async fn readdir(&mut self, id: u32, _: String) -> Result<Name, Self::Error> {
        let files = match self.1 {
            Response::Empty => Vec::new(),
            Response::Count => vec![File::dummy("file"); 2_000],
            Response::Bytes => vec![File::dummy("x".repeat(60_000)); 3],
            Response::Slow => {
                tokio::time::sleep(Duration::from_secs(31)).await;
                vec![File::dummy("file")]
            }
        };
        Ok(Name { id, files })
    }

    async fn close(&mut self, id: u32, _: String) -> Result<Status, Self::Error> {
        self.0.store(true, Ordering::SeqCst);
        Ok(Status {
            id,
            status_code: StatusCode::Ok,
            error_message: String::new(),
            language_tag: String::new(),
        })
    }
}

#[tokio::test]
async fn malformed_directory_response_closes_the_remote_handle() {
    assert!(matches!(
        failure(Response::Empty).await,
        Error::UnexpectedBehavior(_)
    ));
}

#[tokio::test]
async fn oversized_directory_response_stream_closes_the_remote_handle() {
    assert!(matches!(failure(Response::Bytes).await, Error::Limited(_)));
}

#[tokio::test]
async fn excessive_directory_entry_stream_closes_the_remote_handle() {
    assert!(matches!(failure(Response::Count).await, Error::Limited(_)));
}

#[tokio::test(start_paused = true)]
async fn total_directory_timeout_closes_the_remote_handle() {
    let start = tokio::time::Instant::now();
    assert!(matches!(failure(Response::Slow).await, Error::Timeout));
    assert_eq!(start.elapsed(), Duration::from_secs(31));
}

async fn failure(response: Response) -> Error {
    let (client, stream) = tokio::io::duplex(1024);
    let closed = Arc::new(AtomicBool::new(false));
    let server = tokio::spawn(server::run(
        stream,
        EmptyDirectory(closed.clone(), response),
    ));
    let session = SftpSession::new(client).await.unwrap();
    session.set_timeout(60);
    let error = match session.read_dir("/").await {
        Err(error) => error,
        Ok(_) => panic!("Unbounded directory read succeeded"),
    };
    assert!(closed.load(Ordering::SeqCst));
    session.close().await.unwrap();
    server.abort();
    error
}
