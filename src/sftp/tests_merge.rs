use tokio::sync::mpsc;

use super::{
    ConflictChoice, Direction, TransferEvent,
    tests::{connected, directory, finished},
    tests_conflicts::conflict,
};

#[tokio::test]
async fn merge_both_directions_recurses_and_duplicates_files_and_type_mismatches() {
    for uploading in [false, true] {
        let (_fixture, _control, browser) = connected().await;
        let root = directory();
        let source = root.join("source");
        let target = root.join("target");
        for path in [
            source.join("deep"),
            source.join("folder-collision"),
            source.join("empty"),
            target.join("deep"),
            target.join("file-collision"),
        ] {
            tokio::fs::create_dir_all(path).await.unwrap();
        }
        for (path, contents) in [
            (source.join("deep/nested.txt"), "New nested"),
            (source.join("new.txt"), "New file"),
            (source.join("file-collision"), "New file collision"),
            (
                source.join("folder-collision/data.txt"),
                "New folder collision",
            ),
            (target.join("deep/nested.txt"), "Original nested"),
            (target.join("deep/nested (1).txt"), "Original numbered"),
            (target.join("folder-collision"), "Original file collision"),
            (
                target.join("file-collision/keep.txt"),
                "Original folder collision",
            ),
        ] {
            tokio::fs::write(path, contents).await.unwrap();
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
        assert!(request.can_merge);
        assert!(transfer.resolve_conflict(request.id, ConflictChoice::Merge));
        assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
        for (path, expected) in [
            ("deep/nested.txt", "Original nested"),
            ("deep/nested (1).txt", "Original numbered"),
            ("deep/nested (2).txt", "New nested"),
            ("new.txt", "New file"),
            ("folder-collision", "Original file collision"),
            ("folder-collision (1)/data.txt", "New folder collision"),
            ("file-collision/keep.txt", "Original folder collision"),
            ("file-collision (1)", "New file collision"),
        ] {
            let contents = if uploading {
                browser
                    .session
                    .read(format!("{remote}/{path}"))
                    .await
                    .unwrap()
            } else {
                tokio::fs::read(target.join(path)).await.unwrap()
            };
            assert_eq!(
                contents,
                expected.as_bytes(),
                "{path}, uploading={uploading}"
            );
        }
        let empty_is_dir = if uploading {
            browser
                .session
                .symlink_metadata(format!("{remote}/empty"))
                .await
                .unwrap()
                .file_type()
                .is_dir()
        } else {
            target.join("empty").is_dir()
        };
        assert!(empty_is_dir);
        let total = [
            "New nested",
            "New file",
            "New file collision",
            "New folder collision",
        ]
        .iter()
        .map(|text| text.len() as u64)
        .sum::<u64>();
        assert!(
            matches!(transfer.progress_event(), Some(TransferEvent::Progress { completed, total: Some(expected), .. }) if completed == total && expected == total)
        );
    }
}

#[tokio::test]
async fn duplicate_folder_both_directions_uses_first_available_number_and_preserves_existing_trees()
{
    for uploading in [false, true] {
        let (_fixture, _control, browser) = connected().await;
        let root = directory();
        let source = root.join("source");
        tokio::fs::create_dir(&source).await.unwrap();
        tokio::fs::write(source.join("new.txt"), b"New source")
            .await
            .unwrap();
        for name in ["Folder.ext", "Folder.ext (1)"] {
            tokio::fs::create_dir(root.join(name)).await.unwrap();
            tokio::fs::write(root.join(name).join("original.txt"), b"Keep original")
                .await
                .unwrap();
        }
        if uploading {
            let (events, mut incoming) = mpsc::channel(32);
            let _seed = browser.transfer_many(
                ["Folder.ext", "Folder.ext (1)"]
                    .map(|name| Direction::Upload {
                        local: root.join(name),
                        remote: format!("/{name}"),
                    })
                    .into(),
                events,
            );
            assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
        } else {
            let (events, mut incoming) = mpsc::channel(32);
            let _seed = browser.transfer_many(
                vec![Direction::Upload {
                    local: source.clone(),
                    remote: "/source".into(),
                }],
                events,
            );
            assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
        }
        let (events, mut incoming) = mpsc::channel(32);
        let transfer = browser.transfer_interactive(
            if uploading {
                Direction::Upload {
                    local: source,
                    remote: "/Folder.ext".into(),
                }
            } else {
                Direction::Download {
                    remote: "/source".into(),
                    local: root.join("Folder.ext"),
                }
            },
            events,
        );
        let request = conflict(&transfer).await;
        assert!(transfer.resolve_conflict(request.id, ConflictChoice::Duplicate));
        assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
        let contents = if uploading {
            browser
                .session
                .read("/Folder.ext (2)/new.txt")
                .await
                .unwrap()
        } else {
            tokio::fs::read(root.join("Folder.ext (2)/new.txt"))
                .await
                .unwrap()
        };
        assert_eq!(contents, b"New source");
        for name in ["Folder.ext", "Folder.ext (1)"] {
            let contents = if uploading {
                browser
                    .session
                    .read(format!("/{name}/original.txt"))
                    .await
                    .unwrap()
            } else {
                tokio::fs::read(root.join(name).join("original.txt"))
                    .await
                    .unwrap()
            };
            assert_eq!(contents, b"Keep original");
        }
    }
}
