use std::{path::PathBuf, sync::atomic::Ordering, time::Duration};

use tokio::sync::mpsc;
use uuid::Uuid;

use super::{Browser, Direction, Transfer, TransferEvent, tests_server};
use crate::ssh::{self, fixture::Fixture};

fn subsystem(stream: russh::ChannelStream<russh::server::Msg>) {
    tokio::spawn(tests_server::serve(stream));
}

pub(super) fn directory() -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/latch-plan/sftp-tests")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

pub(super) async fn connected() -> (Fixture, ssh::SessionControl, Browser) {
    connected_with(subsystem).await
}

pub(super) async fn connected_with(
    subsystem: fn(russh::ChannelStream<russh::server::Msg>),
) -> (Fixture, ssh::SessionControl, Browser) {
    let fixture = Fixture::start_with(false, Some(subsystem)).await;
    let (prompts, mut incoming) = mpsc::channel(8);
    let (events, mut output) = mpsc::channel(32);
    let config = ssh::ConnectionConfig {
        host: "127.0.0.1".into(),
        port: fixture.port,
        username: "synthetic".into(),
        auth: ssh::AuthMethod::Password,
        known_hosts: directory().join("known_hosts"),
        cols: 80,
        rows: 24,
    };
    let control = ssh::spawn(config, prompts, events);
    tokio::spawn(async move {
        while let Some(prompt) = incoming.recv().await {
            match prompt {
                ssh::Prompt::HostKey { reply, .. } => {
                    let _ = reply.send(true);
                }
                ssh::Prompt::Secret { reply, .. } => {
                    let _ = reply.send(Some(ssh::Secret::new("test-only-password".into())));
                }
                _ => panic!("Unexpected synthetic authentication challenge"),
            }
        }
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = output.recv().await {
            match event {
                ssh::Event::Status(ssh::State::Connected) => return,
                ssh::Event::Error(error) => panic!("{error}"),
                _ => (),
            }
        }
        panic!("SSH event stream closed before connection");
    })
    .await
    .unwrap();
    tokio::spawn(async move { while output.recv().await.is_some() {} });
    let browser = Browser::connect(&control.commands).await.unwrap();
    (fixture, control, browser)
}

pub(super) async fn finished(events: &mut mpsc::Receiver<TransferEvent>) -> TransferEvent {
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(event) = events.recv().await {
            if !matches!(event, TransferEvent::Progress { .. }) {
                return event;
            }
        }
        panic!("Transfer closed without terminal state");
    })
    .await
    .unwrap()
}

pub(super) fn transfer(
    browser: &Browser,
    direction: Direction,
) -> (Transfer, mpsc::Receiver<TransferEvent>) {
    let (events, receiver) = mpsc::channel(32);
    (browser.transfer(direction, events), receiver)
}

#[tokio::test]
async fn browse_navigate_upload_download_and_reuse_authenticated_transport() {
    let (fixture, _control, browser) = connected().await;
    let root = browser.list(".").await.unwrap();
    assert_eq!(
        (
            &root.path,
            root.entries[0].name.as_str(),
            root.entries[0].is_dir
        ),
        (&"/".to_owned(), "folder", true)
    );
    assert_eq!(
        browser.list("/folder").await.unwrap().entries[0].name,
        "nested.txt"
    );
    let directory = directory();
    let original: Vec<_> = (0..200_000).map(|index| (index % 251) as u8).collect();
    let local = directory.join("source.bin");
    tokio::fs::write(&local, &original).await.unwrap();
    let (_upload, mut events) = transfer(
        &browser,
        Direction::Upload {
            local,
            remote: "/uploaded.bin".into(),
        },
    );
    assert!(matches!(
        events.recv().await,
        Some(TransferEvent::Progress {
            total: Some(200_000),
            ..
        })
    ));
    assert_eq!(finished(&mut events).await, TransferEvent::Completed);
    let destination = directory.join("downloaded.bin");
    let (_download, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/uploaded.bin".into(),
            local: destination.clone(),
        },
    );
    assert_eq!(finished(&mut events).await, TransferEvent::Completed);
    assert_eq!(tokio::fs::read(destination).await.unwrap(), original);
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.counts.ptys.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_retains_remote_and_local_partial_files() {
    let (_fixture, _control, browser) = connected().await;
    let directory = directory();
    let local = directory.join("large.bin");
    tokio::fs::write(&local, vec![0x42; 8 * 1024 * 1024])
        .await
        .unwrap();
    let (upload, mut events) = transfer(
        &browser,
        Direction::Upload {
            local,
            remote: "/cancelled.bin".into(),
        },
    );
    assert!(matches!(
        events.recv().await,
        Some(TransferEvent::Progress { .. })
    ));
    upload.cancel();
    assert_eq!(finished(&mut events).await, TransferEvent::Cancelled);
    let entries = browser.list("/").await.unwrap().entries;
    assert!(entries.iter().any(|entry| entry.name == "cancelled.bin"));
    assert!(
        !entries
            .iter()
            .any(|entry| entry.name.starts_with(".latch-"))
    );
    let destination = directory.join("cancelled-download.bin");
    let (download, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/large.bin".into(),
            local: destination.clone(),
        },
    );
    assert!(matches!(
        events.recv().await,
        Some(TransferEvent::Progress { .. })
    ));
    drop(download);
    assert_eq!(finished(&mut events).await, TransferEvent::Cancelled);
    assert!(destination.is_file());
    assert!(!std::fs::read_dir(directory).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".latch-")
    }));
}

#[tokio::test]
async fn existing_destinations_and_transfer_failures_preserve_data() {
    let (_fixture, _control, browser) = connected().await;
    let directory = directory();
    let local = directory.join("existing.txt");
    tokio::fs::write(&local, b"Preserve local data")
        .await
        .unwrap();
    let (_download, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/hello.txt".into(),
            local: local.clone(),
        },
    );
    assert!(
        matches!(finished(&mut events).await, TransferEvent::Failed(message) if message.contains("preserved"))
    );
    assert_eq!(
        tokio::fs::read(&local).await.unwrap(),
        b"Preserve local data"
    );
    let (_upload, mut events) = transfer(
        &browser,
        Direction::Upload {
            local,
            remote: "/hello.txt".into(),
        },
    );
    assert!(
        matches!(finished(&mut events).await, TransferEvent::Failed(message) if message.contains("preserved"))
    );
    assert_eq!(
        browser.session.read("/hello.txt").await.unwrap(),
        b"Synthetic SFTP content\n"
    );
    let (_missing, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/missing".into(),
            local: directory.join("missing.txt"),
        },
    );
    assert!(matches!(
        finished(&mut events).await,
        TransferEvent::Failed(_)
    ));
    assert!(!directory.join("missing.txt").exists());
}
