use super::*;
use crate::sftp::{ConflictChoice, Transfer, tests::finished, tests_conflicts::conflict};

async fn copying_large_file(transfer: &Transfer) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if matches!(transfer.progress_event(), Some(TransferEvent::Progress { name, completed, .. }) if name == "large.bin" && completed > 1024) {
                return;
            }
            if let Some(event) = transfer.terminal_event() {
                panic!("Transfer ended before cancellation: {event:?}");
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn interactive_folder_replace_upload_and_download_publish_only_the_new_tree() {
    let (_fixture, _control, browser) = connected().await;
    let source = directory().join("source");
    tokio::fs::create_dir(&source).await.unwrap();
    tokio::fs::write(source.join("new.txt"), b"New tree")
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_interactive(
        Direction::Upload {
            local: source,
            remote: "/folder".into(),
        },
        events,
    );
    let request = conflict(&transfer).await;
    assert!(transfer.resolve_conflict(request.id, ConflictChoice::Replace));
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    assert_eq!(browser.list("/folder").await.unwrap().entries.len(), 1);
    assert_eq!(
        browser.session.read("/folder/new.txt").await.unwrap(),
        b"New tree"
    );

    let local = directory().join("destination");
    tokio::fs::create_dir(&local).await.unwrap();
    tokio::fs::write(local.join("old.txt"), b"Old tree")
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_interactive(
        Direction::Download {
            remote: "/folder".into(),
            local: local.clone(),
        },
        events,
    );
    let request = conflict(&transfer).await;
    assert!(transfer.resolve_conflict(request.id, ConflictChoice::Replace));
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    assert!(!local.join("old.txt").exists());
    assert_eq!(
        tokio::fs::read(local.join("new.txt")).await.unwrap(),
        b"New tree"
    );
    assert_eq!(
        std::fs::read_dir(local.parent().unwrap()).unwrap().count(),
        1
    );
}

#[tokio::test]
async fn cancelled_folder_replacement_upload_keeps_original_and_partial_tree() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("folder");
    tokio::fs::create_dir(&local).await.unwrap();
    tokio::fs::write(local.join("large.bin"), vec![0x42; 8 * 1024 * 1024])
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_interactive(
        Direction::Upload {
            local,
            remote: "/folder".into(),
        },
        events,
    );
    let request = conflict(&transfer).await;
    assert!(transfer.resolve_conflict(request.id, ConflictChoice::Replace));
    copying_large_file(&transfer).await;
    let error = transfer.cancel_and_wait().await.unwrap_err();
    let Error::PartialRetained { path, .. } = error else {
        panic!("Missing partial folder path")
    };
    assert_eq!(
        browser.session.read("/folder/nested.txt").await.unwrap(),
        b"Nested synthetic content\n"
    );
    let partial = browser
        .session
        .read(format!("{path}/large.bin"))
        .await
        .unwrap();
    assert!(!partial.is_empty() && partial.len() < 8 * 1024 * 1024);
    assert!(
        matches!(finished(&mut incoming).await, TransferEvent::Failed(message) if message.contains(&path))
    );
}

#[tokio::test]
async fn cancelled_folder_replacement_download_keeps_original_and_partial_tree() {
    let (_fixture, _control, browser) = connected().await;
    browser
        .session
        .rename("/large.bin", "/folder/large.bin")
        .await
        .unwrap();
    let local = directory().join("folder");
    tokio::fs::create_dir(&local).await.unwrap();
    tokio::fs::write(local.join("old.txt"), b"Original tree")
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_interactive(
        Direction::Download {
            remote: "/folder".into(),
            local: local.clone(),
        },
        events,
    );
    let request = conflict(&transfer).await;
    assert!(transfer.resolve_conflict(request.id, ConflictChoice::Replace));
    copying_large_file(&transfer).await;
    let error = transfer.cancel_and_wait().await.unwrap_err();
    let Error::PartialRetained { path, .. } = error else {
        panic!("Missing partial folder path")
    };
    assert_eq!(
        tokio::fs::read(local.join("old.txt")).await.unwrap(),
        b"Original tree"
    );
    let partial = tokio::fs::read(std::path::Path::new(&path).join("large.bin"))
        .await
        .unwrap();
    assert!(!partial.is_empty() && partial.len() < 16 * 1024 * 1024);
    assert!(
        matches!(finished(&mut incoming).await, TransferEvent::Failed(message) if message.contains(&path))
    );
}
