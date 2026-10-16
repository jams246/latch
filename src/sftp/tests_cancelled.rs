use std::time::Duration;

use tokio::{io::AsyncWriteExt, sync::mpsc};

use super::{
    Direction, Error, TransferEvent,
    tests::{connected, directory, finished, transfer},
};

async fn wait_for_bytes(events: &mut mpsc::Receiver<TransferEvent>) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = events.recv().await {
            if matches!(event, TransferEvent::Progress { completed, .. } if completed > 0) {
                return;
            }
            assert!(matches!(event, TransferEvent::Progress { .. }), "{event:?}");
        }
        panic!("Transfer closed before copying bytes");
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn cancelled_upload_keeps_partial_file_at_intended_destination() {
    let (_fixture, _control, browser) = connected().await;
    let source = directory().join("source.bin");
    tokio::fs::write(&source, vec![0x42; 8 * 1024 * 1024])
        .await
        .unwrap();
    let (upload, mut events) = transfer(
        &browser,
        Direction::Upload {
            local: source,
            remote: "/partial-upload.bin".into(),
        },
    );
    wait_for_bytes(&mut events).await;
    upload.cancel_and_wait().await.unwrap();
    assert_eq!(finished(&mut events).await, TransferEvent::Cancelled);
    let partial = browser.session.read("/partial-upload.bin").await.unwrap();
    assert!(!partial.is_empty() && partial.len() < 8 * 1024 * 1024);
    assert!(partial.iter().all(|byte| *byte == 0x42));
    assert!(
        browser
            .list("/")
            .await
            .unwrap()
            .entries
            .iter()
            .all(|entry| !entry.name.starts_with(".latch-"))
    );
}

#[tokio::test]
async fn cancelled_download_keeps_partial_file_at_intended_destination() {
    let (_fixture, _control, browser) = connected().await;
    let directory = directory();
    let destination = directory.join("partial-download.bin");
    let (download, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/large.bin".into(),
            local: destination.clone(),
        },
    );
    wait_for_bytes(&mut events).await;
    download.cancel_and_wait().await.unwrap();
    assert_eq!(finished(&mut events).await, TransferEvent::Cancelled);
    let partial = tokio::fs::read(destination).await.unwrap();
    assert!(!partial.is_empty() && partial.len() < 16 * 1024 * 1024);
    assert!(partial.iter().all(|byte| *byte == 0x5a));
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1);
}

#[tokio::test]
async fn cancelled_batch_keeps_completed_file_and_current_folder_without_starting_later_files() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory();
    let completed = b"Completed before cancellation";
    tokio::fs::write(local.join("first.txt"), completed)
        .await
        .unwrap();
    tokio::fs::create_dir(local.join("folder")).await.unwrap();
    tokio::fs::write(local.join("folder/large.bin"), vec![0x42; 8 * 1024 * 1024])
        .await
        .unwrap();
    tokio::fs::write(local.join("later.txt"), b"Must not start")
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let copy = browser.transfer_many(
        vec![
            Direction::Upload {
                local: local.join("first.txt"),
                remote: "/completed.txt".into(),
            },
            Direction::Upload {
                local: local.join("folder"),
                remote: "/partial-folder".into(),
            },
            Direction::Upload {
                local: local.join("later.txt"),
                remote: "/later.txt".into(),
            },
        ],
        events,
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = incoming.recv().await {
            if matches!(&event, TransferEvent::Progress { name, completed: bytes, .. }
                if name == "large.bin" && *bytes > completed.len() as u64)
            {
                return;
            }
            assert!(matches!(event, TransferEvent::Progress { .. }), "{event:?}");
        }
        panic!("Transfer closed before copying the large file");
    })
    .await
    .unwrap();
    copy.cancel_and_wait().await.unwrap();
    assert_eq!(finished(&mut incoming).await, TransferEvent::Cancelled);
    assert_eq!(
        browser.session.read("/completed.txt").await.unwrap(),
        completed
    );
    let entries = browser.list("/partial-folder").await.unwrap().entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "large.bin");
    assert!(
        entries[0]
            .size
            .is_some_and(|size| size > 0 && size < 8 * 1024 * 1024)
    );
    assert!(
        browser
            .list("/")
            .await
            .unwrap()
            .entries
            .iter()
            .all(|entry| entry.name != "later.txt" && !entry.name.starts_with(".latch-"))
    );
}

#[tokio::test]
async fn cancelled_upload_conflict_preserves_destination_and_partial_temporary_file() {
    let (_fixture, _control, browser) = connected().await;
    let source = directory().join("source.bin");
    tokio::fs::write(&source, vec![0x42; 8 * 1024 * 1024])
        .await
        .unwrap();
    let (upload, mut events) = transfer(
        &browser,
        Direction::Upload {
            local: source,
            remote: "/upload-conflict.bin".into(),
        },
    );
    wait_for_bytes(&mut events).await;
    let mut concurrent = browser
        .session
        .create("/upload-conflict.bin")
        .await
        .unwrap();
    concurrent
        .write_all(b"Preserve concurrent data")
        .await
        .unwrap();
    concurrent.close().await.unwrap();
    let error = upload.cancel_and_wait().await.unwrap_err();
    let Error::PartialRetained { path, .. } = error else {
        panic!("Expected retained partial diagnostic, got {error:?}");
    };
    assert!(path.starts_with("/.latch-") && path.ends_with(".part"));
    assert_eq!(
        browser.session.read("/upload-conflict.bin").await.unwrap(),
        b"Preserve concurrent data"
    );
    let partial = browser.session.read(path).await.unwrap();
    assert!(!partial.is_empty() && partial.len() < 8 * 1024 * 1024);
    assert!(partial.iter().all(|byte| *byte == 0x42));
    assert!(
        matches!(finished(&mut events).await, TransferEvent::Failed(message) if message.contains("Partial data remains"))
    );
}

#[tokio::test]
async fn cancelled_download_conflict_preserves_destination_and_partial_temporary_file() {
    let (_fixture, _control, browser) = connected().await;
    let directory = directory();
    let destination = directory.join("download-conflict.bin");
    let (download, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/large.bin".into(),
            local: destination.clone(),
        },
    );
    wait_for_bytes(&mut events).await;
    tokio::fs::write(&destination, b"Preserve concurrent data")
        .await
        .unwrap();
    let error = download.cancel_and_wait().await.unwrap_err();
    let Error::PartialRetained { path, reason } = error else {
        panic!("Expected retained partial diagnostic, got {error:?}");
    };
    assert!(reason.contains("Existing destination was preserved"));
    assert_eq!(
        tokio::fs::read(destination).await.unwrap(),
        b"Preserve concurrent data"
    );
    let partial = tokio::fs::read(path).await.unwrap();
    assert!(!partial.is_empty() && partial.len() < 16 * 1024 * 1024);
    assert!(partial.iter().all(|byte| *byte == 0x5a));
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 2);
    assert!(
        matches!(finished(&mut events).await, TransferEvent::Failed(message) if message.contains("Partial data remains"))
    );
}
