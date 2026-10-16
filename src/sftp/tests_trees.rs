use tokio::sync::mpsc;

use super::{
    Direction, TransferEvent,
    tests::{connected, directory, finished},
};

#[tokio::test]
async fn selected_folder_and_file_upload_and_download_preserve_nested_contents() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory();
    tokio::fs::create_dir_all(local.join("source/empty"))
        .await
        .unwrap();
    tokio::fs::write(local.join("source/nested.txt"), b"Nested copy content")
        .await
        .unwrap();
    tokio::fs::write(local.join("file.txt"), b"File copy content")
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let _upload = browser.transfer_many(
        vec![
            Direction::Upload {
                local: local.join("source"),
                remote: "/copied".into(),
            },
            Direction::Upload {
                local: local.join("file.txt"),
                remote: "/copied-file.txt".into(),
            },
        ],
        events,
    );
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    let (events, mut incoming) = mpsc::channel(32);
    let _download = browser.transfer_many(
        vec![
            Direction::Download {
                remote: "/copied".into(),
                local: local.join("downloaded"),
            },
            Direction::Download {
                remote: "/copied-file.txt".into(),
                local: local.join("downloaded-file.txt"),
            },
        ],
        events,
    );
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    assert_eq!(
        tokio::fs::read(local.join("downloaded/nested.txt"))
            .await
            .unwrap(),
        b"Nested copy content"
    );
    assert_eq!(
        tokio::fs::read(local.join("downloaded-file.txt"))
            .await
            .unwrap(),
        b"File copy content"
    );
    assert!(local.join("downloaded/empty").is_dir());
}

#[tokio::test]
async fn copy_preflight_rejects_nested_links_before_creating_destinations() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory();
    let (events, mut incoming) = mpsc::channel(32);
    let _copy = browser.transfer_many(
        vec![
            Direction::Download {
                remote: "/hello.txt".into(),
                local: local.join("first.txt"),
            },
            Direction::Download {
                remote: "/other".into(),
                local: local.join("other"),
            },
        ],
        events,
    );
    assert!(
        matches!(finished(&mut incoming).await, TransferEvent::Failed(message) if message.contains("Links and special files"))
    );
    assert!(std::fs::read_dir(local).unwrap().next().is_none());
}

#[tokio::test]
async fn copy_preflight_preserves_existing_destinations_before_any_copy() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory();
    tokio::fs::write(local.join("existing.txt"), b"Keep existing content")
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let _copy = browser.transfer_many(
        vec![
            Direction::Download {
                remote: "/folder".into(),
                local: local.join("new-folder"),
            },
            Direction::Download {
                remote: "/hello.txt".into(),
                local: local.join("existing.txt"),
            },
        ],
        events,
    );
    assert!(
        matches!(finished(&mut incoming).await, TransferEvent::Failed(message) if message.contains("preserved"))
    );
    assert!(!local.join("new-folder").exists());
    assert_eq!(
        tokio::fs::read(local.join("existing.txt")).await.unwrap(),
        b"Keep existing content"
    );
}

#[tokio::test]
async fn copy_selected_link_never_downloads_its_target() {
    let (_fixture, _control, browser) = connected().await;
    let destination = directory().join("link-target.txt");
    let (events, mut incoming) = mpsc::channel(32);
    let _copy = browser.transfer_many(
        vec![Direction::Download {
            remote: "/file-link".into(),
            local: destination.clone(),
        }],
        events,
    );
    assert!(
        matches!(finished(&mut incoming).await, TransferEvent::Failed(message) if message.contains("No link target was followed"))
    );
    assert!(!destination.exists());
}

#[tokio::test]
async fn cancelling_batch_copy_cleans_temporary_file_before_reporting_completion() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory();
    tokio::fs::create_dir(local.join("source")).await.unwrap();
    tokio::fs::write(local.join("source/large.bin"), vec![0x42; 8 * 1024 * 1024])
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let copy = browser.transfer_many(
        vec![Direction::Upload {
            local: local.join("source"),
            remote: "/cancelled-folder".into(),
        }],
        events,
    );
    assert!(matches!(
        incoming.recv().await,
        Some(TransferEvent::Progress { .. })
    ));
    copy.cancel_and_wait().await.unwrap();
    assert_eq!(finished(&mut incoming).await, TransferEvent::Cancelled);
    assert!(
        browser
            .list("/cancelled-folder")
            .await
            .unwrap()
            .entries
            .is_empty()
    );
}
