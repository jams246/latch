use super::*;

#[tokio::test]
async fn explicitly_opened_remote_file_link_downloads_target_while_selected_links_stay_rejected() {
    let (_fixture, _control, browser) = connected().await;
    let root = directory();
    let (events, mut incoming) = mpsc::channel(32);
    let _transfer = browser.transfer_interactive(
        Direction::Download {
            remote: "/file-link".into(),
            local: root.join("explicit.txt"),
        },
        events,
    );
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    assert_eq!(
        tokio::fs::read(root.join("explicit.txt")).await.unwrap(),
        b"Synthetic SFTP content\n"
    );
    let (events, mut incoming) = mpsc::channel(32);
    let _transfer = browser.transfer_many_interactive(
        vec![Direction::Download {
            remote: "/file-link".into(),
            local: root.join("selected.txt"),
        }],
        events,
    );
    assert!(
        matches!(finished(&mut incoming).await, TransferEvent::Failed(message) if message.contains("No link target was followed"))
    );
    assert!(!root.join("selected.txt").exists());
}

#[tokio::test]
async fn remote_destination_link_cannot_merge_and_duplicate_never_changes_its_target() {
    let (_fixture, _control, browser) = connected().await;
    let source = directory().join("source");
    tokio::fs::create_dir(&source).await.unwrap();
    tokio::fs::write(source.join("new.txt"), b"New source")
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_interactive(
        Direction::Upload {
            local: source,
            remote: "/folder-link".into(),
        },
        events,
    );
    let request = conflict(&transfer).await;
    assert!(request.is_dir);
    assert!(!request.can_merge);
    assert!(!transfer.resolve_conflict(request.id, ConflictChoice::Merge));
    assert!(transfer.resolve_conflict(request.id, ConflictChoice::Duplicate));
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    assert_eq!(browser.list("/folder").await.unwrap().entries.len(), 1);
    assert_eq!(
        browser
            .session
            .read("/folder-link (1)/new.txt")
            .await
            .unwrap(),
        b"New source"
    );
}

#[tokio::test]
async fn local_destination_link_cannot_merge_and_source_link_is_rejected_before_any_change() {
    let (_fixture, _control, browser) = connected().await;
    let root = directory();
    let original = root.join("original");
    let link = root.join("link");
    std::fs::create_dir(&original).unwrap();
    std::fs::write(original.join("keep.txt"), b"Keep original").unwrap();
    if let Err(error) = std::os::windows::fs::symlink_dir(&original, &link) {
        if error.raw_os_error() == Some(1314) {
            return;
        }
        panic!("Cannot create synthetic directory link: {error}");
    }
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_interactive(
        Direction::Download {
            remote: "/folder".into(),
            local: link.clone(),
        },
        events,
    );
    let request = conflict(&transfer).await;
    assert!(!request.can_merge);
    assert!(transfer.resolve_conflict(request.id, ConflictChoice::Duplicate));
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    assert_eq!(std::fs::read_dir(&original).unwrap().count(), 1);
    assert_eq!(
        tokio::fs::read(root.join("link (1)/nested.txt"))
            .await
            .unwrap(),
        b"Nested synthetic content\n"
    );
    let (events, mut incoming) = mpsc::channel(32);
    let _transfer = browser.transfer_interactive(
        Direction::Upload {
            local: link,
            remote: "/copy-link".into(),
        },
        events,
    );
    assert!(
        matches!(finished(&mut incoming).await, TransferEvent::Failed(message) if message.contains("No link target was followed"))
    );
    assert!(
        browser
            .session
            .symlink_metadata("/copy-link")
            .await
            .is_err()
    );
}
