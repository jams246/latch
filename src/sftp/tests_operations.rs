use super::{Error, tests::connected};

#[tokio::test]
async fn rename_file_and_folder_preserves_contents_and_existing_destinations() {
    let (_fixture, _control, browser) = connected().await;
    browser.rename("/hello.txt", "renamed.txt").await.unwrap();
    browser.rename("/folder", "renamed-folder").await.unwrap();
    assert_eq!(
        browser.session.read("/renamed.txt").await.unwrap(),
        b"Synthetic SFTP content\n"
    );
    assert_eq!(
        browser.list("/renamed-folder").await.unwrap().entries[0].name,
        "nested.txt"
    );
    assert!(matches!(
        browser.rename("/renamed.txt", "large.bin").await,
        Err(Error::DestinationExists)
    ));
    assert_eq!(
        browser.session.read("/renamed.txt").await.unwrap(),
        b"Synthetic SFTP content\n"
    );
}

#[tokio::test]
async fn new_folder_rejects_existing_entries_including_broken_links() {
    let (_fixture, _control, browser) = connected().await;
    browser
        .create_directory("/folder", "new folder")
        .await
        .unwrap();
    assert!(
        browser
            .list("/folder/new folder")
            .await
            .unwrap()
            .entries
            .is_empty()
    );
    for name in ["folder", "hello.txt", "broken-link"] {
        assert!(matches!(
            browser.create_directory("/", name).await,
            Err(Error::DestinationExists)
        ));
    }
}

#[tokio::test]
async fn delete_folders_does_not_follow_nested_or_selected_links() {
    let (_fixture, _control, browser) = connected().await;
    browser
        .delete(&[
            "/folder-link".into(),
            "/broken-link".into(),
            "/other".into(),
        ])
        .await
        .unwrap();
    assert_eq!(
        browser.session.read("/folder/nested.txt").await.unwrap(),
        b"Nested synthetic content\n"
    );
    assert_eq!(
        browser.session.read("/hello.txt").await.unwrap(),
        b"Synthetic SFTP content\n"
    );
    let remaining = browser.list("/").await.unwrap();
    assert!(
        !remaining
            .entries
            .iter()
            .any(|entry| matches!(entry.name.as_str(), "folder-link" | "broken-link" | "other"))
    );
}

#[tokio::test]
async fn delete_validates_every_selection_before_removing_anything() {
    let (_fixture, _control, browser) = connected().await;
    for path in ["/", "/.", "/..", "/folder/..", "/folder/", ".", ".."] {
        assert!(matches!(
            browser.delete(&["/hello.txt".into(), path.into()]).await,
            Err(Error::InvalidPath)
        ));
    }
    assert_eq!(
        browser.session.read("/hello.txt").await.unwrap(),
        b"Synthetic SFTP content\n"
    );
}

#[tokio::test]
async fn rename_and_new_folder_reject_traversal_names() {
    let (_fixture, _control, browser) = connected().await;
    for name in ["", ".", "..", "../escape", "a/b", "a\\b", "bad\0name"] {
        assert!(matches!(
            browser.rename("/hello.txt", name).await,
            Err(Error::InvalidPath)
        ));
        assert!(matches!(
            browser.create_directory("/", name).await,
            Err(Error::InvalidPath)
        ));
    }
}
