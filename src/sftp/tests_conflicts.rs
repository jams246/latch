use std::time::Duration;

use tokio::sync::mpsc;

use super::{
    Conflict, ConflictChoice, Direction, Transfer, TransferEvent,
    tests::{connected, directory, finished},
};

mod links;
mod type_replace;

pub(super) async fn conflict(transfer: &Transfer) -> Conflict {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(conflict) = transfer.conflict() {
                return conflict;
            }
            if let Some(event) = transfer.terminal_event() {
                panic!("Expected conflict, received {event:?}");
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn download_file_conflict_skip_replace_and_numbered_duplicate_preserve_expected_contents() {
    for choice in [
        ConflictChoice::Skip,
        ConflictChoice::Replace,
        ConflictChoice::Duplicate,
    ] {
        let (_fixture, _control, browser) = connected().await;
        let local = directory();
        tokio::fs::write(local.join("File.txt"), b"Original")
            .await
            .unwrap();
        tokio::fs::write(local.join("File (1).txt"), b"Number one")
            .await
            .unwrap();
        let (events, mut incoming) = mpsc::channel(32);
        let transfer = browser.transfer_interactive(
            Direction::Download {
                remote: "/hello.txt".into(),
                local: local.join("File.txt"),
            },
            events,
        );
        let request = conflict(&transfer).await;
        assert!(!request.can_merge);
        assert!(!transfer.resolve_conflict(request.id + 1, choice));
        assert!(!transfer.resolve_conflict(request.id, ConflictChoice::Merge));
        assert!(transfer.resolve_conflict(request.id, choice));
        assert!(transfer.conflict().is_none());
        assert!(!transfer.resolve_conflict(request.id, choice));
        assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
        let original = tokio::fs::read(local.join("File.txt")).await.unwrap();
        assert_eq!(
            original,
            if choice == ConflictChoice::Replace {
                b"Synthetic SFTP content\n".as_slice()
            } else {
                b"Original"
            }
        );
        assert_eq!(
            local.join("File (2).txt").exists(),
            choice == ConflictChoice::Duplicate
        );
        if choice == ConflictChoice::Duplicate {
            assert_eq!(
                tokio::fs::read(local.join("File (2).txt")).await.unwrap(),
                b"Synthetic SFTP content\n"
            );
        }
    }
}

#[tokio::test]
async fn upload_file_conflict_skip_replace_and_duplicate_preserve_expected_contents() {
    for choice in [
        ConflictChoice::Skip,
        ConflictChoice::Replace,
        ConflictChoice::Duplicate,
    ] {
        let (_fixture, _control, browser) = connected().await;
        let local = directory().join("File.txt");
        tokio::fs::write(&local, b"Replacement").await.unwrap();
        let (events, mut incoming) = mpsc::channel(32);
        let transfer = browser.transfer_interactive(
            Direction::Upload {
                local,
                remote: "/hello.txt".into(),
            },
            events,
        );
        let request = conflict(&transfer).await;
        assert!(transfer.resolve_conflict(request.id, choice));
        assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
        assert_eq!(
            browser.session.read("/hello.txt").await.unwrap(),
            if choice == ConflictChoice::Replace {
                b"Replacement".as_slice()
            } else {
                b"Synthetic SFTP content\n"
            }
        );
        let duplicates: Vec<_> = browser
            .list("/")
            .await
            .unwrap()
            .entries
            .into_iter()
            .filter(|entry| entry.name == "hello (1).txt")
            .collect();
        assert_eq!(
            duplicates.len(),
            usize::from(choice == ConflictChoice::Duplicate)
        );
        if choice == ConflictChoice::Duplicate {
            assert_eq!(
                browser.session.read("/hello (1).txt").await.unwrap(),
                b"Replacement"
            );
        }
    }
}

#[tokio::test]
async fn cancelling_a_paused_transfer_awaiting_conflict_leaves_destination_and_no_pending_dialog() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("existing.txt");
    tokio::fs::write(&local, b"Original").await.unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_interactive(
        Direction::Download {
            remote: "/hello.txt".into(),
            local: local.clone(),
        },
        events,
    );
    conflict(&transfer).await;
    transfer.pause();
    transfer.cancel();
    assert!(transfer.conflict().is_none());
    assert_eq!(finished(&mut incoming).await, TransferEvent::Cancelled);
    assert_eq!(tokio::fs::read(local).await.unwrap(), b"Original");
}

#[tokio::test]
async fn interactive_preflight_rejects_nested_links_before_prompting_or_changing_any_destination() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory();
    tokio::fs::write(local.join("existing.txt"), b"Original")
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_many_interactive(
        vec![
            Direction::Download {
                remote: "/hello.txt".into(),
                local: local.join("existing.txt"),
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
    assert!(transfer.conflict().is_none());
    assert_eq!(
        tokio::fs::read(local.join("existing.txt")).await.unwrap(),
        b"Original"
    );
    assert!(!local.join("other").exists());
}

#[tokio::test]
async fn skipping_existing_folder_excludes_its_subtree_from_remaining_transfer_total() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory();
    tokio::fs::create_dir(local.join("folder")).await.unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_many_interactive(
        vec![
            Direction::Download {
                remote: "/folder".into(),
                local: local.join("folder"),
            },
            Direction::Download {
                remote: "/hello.txt".into(),
                local: local.join("hello.txt"),
            },
        ],
        events,
    );
    let request = conflict(&transfer).await;
    assert!(request.can_merge);
    assert!(transfer.resolve_conflict(request.id, ConflictChoice::Skip));
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    let bytes = b"Synthetic SFTP content\n".len() as u64;
    assert!(
        matches!(transfer.progress_event(), Some(TransferEvent::Progress { completed, total: Some(total), .. }) if completed == bytes && total == bytes)
    );
    assert!(!local.join("folder/nested.txt").exists());
}
