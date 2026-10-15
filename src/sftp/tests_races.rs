use super::{
    Direction, TransferEvent,
    tests::{connected, directory, finished, transfer},
};

#[tokio::test]
async fn destination_created_during_download_is_never_overwritten() {
    let (_fixture, _control, browser) = connected().await;
    let directory = directory();
    let destination = directory.join("raced-destination.bin");
    let (_download, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/large.bin".into(),
            local: destination.clone(),
        },
    );
    assert!(matches!(
        events.recv().await,
        Some(TransferEvent::Progress { .. })
    ));
    tokio::fs::write(&destination, b"Preserve concurrently created data")
        .await
        .unwrap();
    assert!(
        matches!(finished(&mut events).await, TransferEvent::Failed(message) if message.contains("preserved"))
    );
    assert_eq!(
        tokio::fs::read(destination).await.unwrap(),
        b"Preserve concurrently created data"
    );
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1);
}

#[tokio::test]
async fn closed_connection_reports_actionable_error_without_a_transfer() {
    let (commands, receiver) = tokio::sync::mpsc::channel(1);
    drop(receiver);
    assert!(matches!(
        super::Browser::connect(&commands).await,
        Err(super::Error::NotConnected)
    ));
}
