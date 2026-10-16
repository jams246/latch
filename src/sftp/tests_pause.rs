use std::time::Duration;

use tokio::{io::AsyncWriteExt, sync::mpsc};

use super::{
    Direction, Transfer, TransferEvent,
    tests::{connected, directory, finished, transfer},
};

async fn wait_for_bytes(events: &mut mpsc::Receiver<TransferEvent>, minimum: u64) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = events.recv().await {
            if matches!(event, TransferEvent::Progress { completed, .. } if completed > minimum) {
                return;
            }
            assert!(matches!(event, TransferEvent::Progress { .. }), "{event:?}");
        }
        panic!("Transfer closed before copying bytes");
    })
    .await
    .unwrap();
}

async fn assert_paused(transfer: &Transfer) -> TransferEvent {
    transfer.pause();
    assert!(transfer.is_paused());
    tokio::time::sleep(Duration::from_millis(100)).await;
    let progress = transfer.progress_event().unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(transfer.progress_event(), Some(progress.clone()));
    assert_eq!(transfer.terminal_event(), None);
    progress
}

fn content() -> Vec<u8> {
    (0..2 * 1024 * 1024)
        .map(|index| (index % 251) as u8)
        .collect()
}

#[tokio::test]
async fn upload_and_download_pause_then_resume_without_restarting_or_corrupting_bytes() {
    let (_fixture, _control, browser) = connected().await;
    let directory = directory();
    let original = content();
    let source = directory.join("source.bin");
    tokio::fs::write(&source, &original).await.unwrap();
    let (upload, mut events) = transfer(
        &browser,
        Direction::Upload {
            local: source,
            remote: "/paused.bin".into(),
        },
    );
    wait_for_bytes(&mut events, 0).await;
    let TransferEvent::Progress { completed, .. } = assert_paused(&upload).await else {
        panic!("Expected paused progress");
    };
    upload.resume();
    assert!(!upload.is_paused());
    wait_for_bytes(&mut events, completed).await;
    assert_paused(&upload).await;
    upload.resume();
    assert_eq!(finished(&mut events).await, TransferEvent::Completed);
    assert_eq!(browser.session.read("/paused.bin").await.unwrap(), original);
    assert!(
        matches!(upload.progress_event(), Some(TransferEvent::Progress {
        completed, total: Some(total), ..
    }) if completed == original.len() as u64 && completed == total)
    );
    let destination = directory.join("download.bin");
    let (download, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/paused.bin".into(),
            local: destination.clone(),
        },
    );
    wait_for_bytes(&mut events, 0).await;
    assert_paused(&download).await;
    download.resume();
    assert_eq!(finished(&mut events).await, TransferEvent::Completed);
    assert_eq!(tokio::fs::read(destination).await.unwrap(), original);
    upload.cancel_and_wait().await.unwrap();
    download.cancel_and_wait().await.unwrap();
}

#[tokio::test]
async fn paused_folder_transfer_preserves_aggregate_progress_and_completed_files() {
    let (_fixture, _control, browser) = connected().await;
    let directory = directory();
    let first = directory.join("first.txt");
    let folder = directory.join("folder");
    tokio::fs::write(&first, b"first").await.unwrap();
    tokio::fs::create_dir(&folder).await.unwrap();
    let original = content();
    tokio::fs::write(folder.join("large.bin"), &original)
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let upload = browser.transfer_many(
        vec![
            Direction::Upload {
                local: first,
                remote: "/first.txt".into(),
            },
            Direction::Upload {
                local: folder,
                remote: "/paused-folder".into(),
            },
        ],
        events,
    );
    wait_for_bytes(&mut incoming, 5).await;
    let paused = assert_paused(&upload).await;
    assert!(matches!(paused, TransferEvent::Progress {
        name, completed, total: Some(total),
    } if name == "large.bin" && completed > 5 && completed < total && total == original.len() as u64 + 5));
    assert_eq!(browser.session.read("/first.txt").await.unwrap(), b"first");
    upload.resume();
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    assert_eq!(
        browser
            .session
            .read("/paused-folder/large.bin")
            .await
            .unwrap(),
        original
    );
    assert!(
        matches!(upload.progress_event(), Some(TransferEvent::Progress {
        completed, total: Some(total), ..
    }) if completed == original.len() as u64 + 5 && completed == total)
    );
    upload.cancel_and_wait().await.unwrap();
}

#[tokio::test]
async fn cancelling_paused_download_wakes_worker_and_preserves_partial_file() {
    let (_fixture, _control, browser) = connected().await;
    let destination = directory().join("partial.bin");
    let (download, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/large.bin".into(),
            local: destination.clone(),
        },
    );
    wait_for_bytes(&mut events, 0).await;
    assert_paused(&download).await;
    tokio::time::timeout(Duration::from_secs(2), download.cancel_and_wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(finished(&mut events).await, TransferEvent::Cancelled);
    let partial = tokio::fs::read(destination).await.unwrap();
    assert!(!partial.is_empty() && partial.len() < 16 * 1024 * 1024);
    assert!(partial.iter().all(|byte| *byte == 0x5a));
}

#[tokio::test]
async fn paused_upload_resume_preserves_destination_created_while_paused() {
    let (_fixture, _control, browser) = connected().await;
    let source = directory().join("source.bin");
    tokio::fs::write(&source, content()).await.unwrap();
    let (upload, mut events) = transfer(
        &browser,
        Direction::Upload {
            local: source,
            remote: "/conflict.bin".into(),
        },
    );
    wait_for_bytes(&mut events, 0).await;
    assert_paused(&upload).await;
    let mut concurrent = browser.session.create("/conflict.bin").await.unwrap();
    concurrent
        .write_all(b"Preserve concurrent data")
        .await
        .unwrap();
    concurrent.close().await.unwrap();
    upload.resume();
    assert!(matches!(
        finished(&mut events).await,
        TransferEvent::Failed(_)
    ));
    assert_eq!(
        browser.session.read("/conflict.bin").await.unwrap(),
        b"Preserve concurrent data"
    );
    assert!(upload.cancel_and_wait().await.is_err());
}
