use super::tests::connected;

#[tokio::test]
async fn list_keeps_server_modification_times_and_link_types_without_folder_sizes() {
    let (_fixture, _control, browser) = connected().await;
    let root = browser.list(".").await.unwrap();
    let folder = root
        .entries
        .iter()
        .find(|entry| entry.name == "folder")
        .unwrap();
    let file = root
        .entries
        .iter()
        .find(|entry| entry.name == "hello.txt")
        .unwrap();
    let link = root
        .entries
        .iter()
        .find(|entry| entry.name == "folder-link")
        .unwrap();

    assert_eq!(
        (folder.is_dir, folder.is_link, folder.size, folder.modified),
        (true, false, None, Some(1_690_000_000))
    );
    assert_eq!(
        (file.is_dir, file.is_link, file.size, file.modified),
        (false, false, Some(23), Some(1_700_000_023))
    );
    assert_eq!(
        (link.is_dir, link.is_link, link.modified),
        (false, true, Some(1_710_000_000))
    );
}

#[tokio::test]
async fn resolve_link_follows_absolute_relative_and_chained_directory_targets() {
    let (_fixture, _control, browser) = connected().await;

    assert_eq!(
        browser
            .resolve_link("/folder-link")
            .await
            .unwrap()
            .as_deref(),
        Some("/folder")
    );
    assert_eq!(
        browser
            .resolve_link("/relative-link")
            .await
            .unwrap()
            .as_deref(),
        Some("/other/deep")
    );
    assert_eq!(
        browser
            .resolve_link("/chain-link")
            .await
            .unwrap()
            .as_deref(),
        Some("/other/deep")
    );
    assert_eq!(
        browser
            .resolve_link("/other/parent-link")
            .await
            .unwrap()
            .as_deref(),
        Some("/")
    );
    let directory = browser.list("/chain-link").await.unwrap();
    assert_eq!(
        (directory.path.as_str(), directory.entries[0].name.as_str()),
        ("/other/deep", "item.csv")
    );
}

#[tokio::test]
async fn resolve_link_returns_none_for_a_regular_file_target() {
    let (_fixture, _control, browser) = connected().await;

    assert_eq!(browser.resolve_link("/file-link").await.unwrap(), None);
}

#[tokio::test]
async fn file_link_download_follows_its_regular_file_target() {
    use super::{
        Direction, TransferEvent,
        tests::{directory, finished, transfer},
    };

    let (_fixture, _control, browser) = connected().await;
    let destination = directory().join("linked-file.txt");
    let (_transfer, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/file-link".into(),
            local: destination.clone(),
        },
    );
    assert_eq!(finished(&mut events).await, TransferEvent::Completed);
    assert_eq!(
        tokio::fs::read(destination).await.unwrap(),
        b"Synthetic SFTP content\n"
    );
}

#[tokio::test]
async fn resolve_link_rejects_broken_links_without_closing_the_browser() {
    let (_fixture, _control, browser) = connected().await;

    assert!(browser.resolve_link("/broken-link").await.is_err());
    assert_eq!(browser.list("/folder").await.unwrap().path, "/folder");
}

#[tokio::test]
async fn resolve_link_rejects_symbolic_link_cycles() {
    let (_fixture, _control, browser) = connected().await;

    assert!(browser.resolve_link("/loop-link").await.is_err());
}

#[tokio::test]
async fn socket_and_block_device_modes_are_not_directories_or_links() {
    let (_fixture, _control, browser) = connected().await;
    let root = browser.list("/").await.unwrap();

    for name in ["socket", "block-device"] {
        let entry = root
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .unwrap();
        assert_eq!(
            (entry.is_dir, entry.is_link, entry.size),
            (false, false, Some(0))
        );
        assert_eq!(browser.resolve_link(&entry.path).await.unwrap(), None);
    }
}
