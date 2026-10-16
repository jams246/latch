use super::*;

#[tokio::test]
async fn replacing_remote_folder_link_never_deletes_its_target() {
    let (_fixture, _control, browser) = connected().await;
    let direction = Direction::Upload {
        local: directory(),
        remote: "/folder-link".into(),
    };
    let replacement = capture(&browser.session, &direction).await.unwrap();
    browser.session.create_dir("/staged").await.unwrap();
    let staged = Direction::Upload {
        local: directory(),
        remote: "/staged".into(),
    };
    publish(&browser.session, &staged, &replacement)
        .await
        .unwrap();
    assert!(
        !browser
            .session
            .symlink_metadata("/folder-link")
            .await
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        browser.session.read("/folder/nested.txt").await.unwrap(),
        b"Nested synthetic content\n"
    );
}

#[tokio::test]
async fn approved_folder_replacements_remove_old_children_after_publication() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("folder");
    tokio::fs::create_dir(&local).await.unwrap();
    tokio::fs::write(local.join("old.txt"), b"Old")
        .await
        .unwrap();
    let direction = Direction::Download {
        remote: "/folder".into(),
        local: local.clone(),
    };
    let replacement = capture(&browser.session, &direction).await.unwrap();
    let staged = local.with_file_name("staged");
    tokio::fs::create_dir(&staged).await.unwrap();
    tokio::fs::write(staged.join("new.txt"), b"New")
        .await
        .unwrap();
    let staged = Direction::Download {
        remote: "/folder".into(),
        local: staged,
    };
    publish(&browser.session, &staged, &replacement)
        .await
        .unwrap();
    assert_eq!(
        tokio::fs::read(local.join("new.txt")).await.unwrap(),
        b"New"
    );
    assert!(!local.join("old.txt").exists());
    let direction = Direction::Upload {
        local,
        remote: "/folder".into(),
    };
    let replacement = capture(&browser.session, &direction).await.unwrap();
    browser.session.create_dir("/staged").await.unwrap();
    let staged = Direction::Upload {
        local: directory(),
        remote: "/staged".into(),
    };
    publish(&browser.session, &staged, &replacement)
        .await
        .unwrap();
    assert!(browser.list("/folder").await.unwrap().entries.is_empty());
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
async fn failed_publication_restores_original_remote_and_local_folders() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("folder");
    tokio::fs::create_dir(&local).await.unwrap();
    tokio::fs::write(local.join("original.txt"), b"Original tree")
        .await
        .unwrap();
    let direction = Direction::Download {
        remote: "/folder".into(),
        local: local.clone(),
    };
    let replacement = capture(&browser.session, &direction).await.unwrap();
    let missing = Direction::Download {
        remote: "/folder".into(),
        local: local.with_file_name("missing"),
    };
    assert!(
        publish(&browser.session, &missing, &replacement)
            .await
            .is_err()
    );
    assert_eq!(
        tokio::fs::read(local.join("original.txt")).await.unwrap(),
        b"Original tree"
    );
    assert_eq!(
        std::fs::read_dir(local.parent().unwrap()).unwrap().count(),
        1
    );

    let direction = Direction::Upload {
        local,
        remote: "/folder".into(),
    };
    let replacement = capture(&browser.session, &direction).await.unwrap();
    let missing = Direction::Upload {
        local: directory(),
        remote: "/missing".into(),
    };
    assert!(
        publish(&browser.session, &missing, &replacement)
            .await
            .is_err()
    );
    assert_eq!(
        browser.session.read("/folder/nested.txt").await.unwrap(),
        b"Nested synthetic content\n"
    );
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
