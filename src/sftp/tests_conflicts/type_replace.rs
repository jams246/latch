use super::*;

#[tokio::test]
async fn replacing_files_with_folders_and_folders_with_files_works_in_both_directions() {
    for uploading in [false, true] {
        for source_is_dir in [false, true] {
            let (_fixture, _control, browser) = connected().await;
            let root = directory();
            let source = root.join("source");
            let target = root.join("target");
            if source_is_dir {
                tokio::fs::create_dir(&source).await.unwrap();
                tokio::fs::write(source.join("new.txt"), b"New source")
                    .await
                    .unwrap();
                tokio::fs::write(&target, b"Old file").await.unwrap();
            } else {
                tokio::fs::write(&source, b"New source").await.unwrap();
                tokio::fs::create_dir(&target).await.unwrap();
                tokio::fs::write(target.join("old.txt"), b"Old folder")
                    .await
                    .unwrap();
            }
            let remote = if uploading { "/target" } else { "/source" };
            let (events, mut incoming) = mpsc::channel(32);
            let _seed = browser.transfer_many(
                vec![Direction::Upload {
                    local: if uploading {
                        target.clone()
                    } else {
                        source.clone()
                    },
                    remote: remote.into(),
                }],
                events,
            );
            assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
            let (events, mut incoming) = mpsc::channel(32);
            let transfer = browser.transfer_interactive(
                if uploading {
                    Direction::Upload {
                        local: source,
                        remote: remote.into(),
                    }
                } else {
                    Direction::Download {
                        remote: remote.into(),
                        local: target.clone(),
                    }
                },
                events,
            );
            let request = conflict(&transfer).await;
            assert!(!request.can_merge);
            assert!(transfer.resolve_conflict(request.id, ConflictChoice::Replace));
            assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
            let contents = if uploading {
                browser
                    .session
                    .read(if source_is_dir {
                        format!("{remote}/new.txt")
                    } else {
                        remote.into()
                    })
                    .await
                    .unwrap()
            } else {
                tokio::fs::read(if source_is_dir {
                    target.join("new.txt")
                } else {
                    target.clone()
                })
                .await
                .unwrap()
            };
            assert_eq!(contents, b"New source");
            let is_dir = if uploading {
                browser
                    .session
                    .symlink_metadata(remote)
                    .await
                    .unwrap()
                    .file_type()
                    .is_dir()
            } else {
                target.is_dir()
            };
            assert_eq!(is_dir, source_is_dir);
        }
    }
}
