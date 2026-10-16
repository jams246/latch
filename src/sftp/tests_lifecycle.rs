use std::time::Duration;

use tokio::sync::mpsc;

use super::{
    Direction, Error, TransferEvent,
    tests::{connected, connected_with, directory, transfer},
};

#[tokio::test]
async fn cancel_and_wait_retains_partial_file_before_transport_is_released() {
    let (_fixture, control, browser) = connected().await;
    let directory = directory();
    let source = directory.join("source.bin");
    tokio::fs::write(&source, vec![0x42; 1024 * 1024])
        .await
        .unwrap();
    let (upload, mut events) = transfer(
        &browser,
        Direction::Upload {
            local: source,
            remote: "/cancel-wait.bin".into(),
        },
    );
    assert!(matches!(
        events.recv().await,
        Some(TransferEvent::Progress { .. })
    ));
    upload.cancel_and_wait().await.unwrap();
    let entries = browser.list("/").await.unwrap().entries;
    assert!(entries.iter().any(|entry| entry.name == "cancel-wait.bin"));
    assert!(
        !entries
            .iter()
            .any(|entry| entry.name.starts_with(".latch-"))
    );
    drop(control);
}

#[tokio::test]
async fn abandoned_full_event_queue_cannot_block_cancel_and_wait() {
    let (_fixture, _control, browser) = connected().await;
    let (events, _unpolled) = mpsc::channel(1);
    events
        .send(TransferEvent::Progress {
            name: "large.bin".into(),
            completed: 0,
            total: None,
        })
        .await
        .unwrap();
    let transfer = browser.transfer(
        Direction::Download {
            remote: "/large.bin".into(),
            local: directory().join("cancelled.bin"),
        },
        events,
    );
    tokio::time::timeout(Duration::from_secs(3), transfer.cancel_and_wait())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn cancel_and_wait_returns_existing_transfer_failure() {
    let (_fixture, _control, browser) = connected().await;
    let directory = directory();
    let destination = directory.join("existing.txt");
    tokio::fs::write(&destination, b"preserve").await.unwrap();
    let (transfer, mut events) = transfer(
        &browser,
        Direction::Download {
            remote: "/hello.txt".into(),
            local: destination.clone(),
        },
    );
    assert!(matches!(
        events.recv().await,
        Some(TransferEvent::Failed(_))
    ));
    assert!(matches!(
        transfer.cancel_and_wait().await,
        Err(Error::DestinationExists)
    ));
    assert_eq!(tokio::fs::read(destination).await.unwrap(), b"preserve");
}

fn slow_close(stream: russh::ChannelStream<russh::server::Msg>) {
    tokio::spawn(super::tests_server::serve_faulty(stream, true, false));
}

fn denied_remove(stream: russh::ChannelStream<russh::server::Msg>) {
    tokio::spawn(super::tests_server::serve_faulty(stream, false, true));
}

#[tokio::test]
async fn remote_close_timeout_reports_retained_local_partial_on_cancellation() {
    let (_fixture, _control, browser) = connected_with(slow_close).await;
    browser.session.set_timeout(1);
    let directory = directory();
    let destination = directory.join("timed-out.bin");
    let (download, mut events) = transfer(
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
    let error = tokio::time::timeout(Duration::from_secs(3), download.cancel_and_wait())
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(error, Error::PartialRetained { .. }));
    assert!(error.to_string().contains("could not be closed"));
    assert!(destination.is_file());
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 1);
}

#[tokio::test]
async fn cancellation_retains_remote_partial_even_when_removal_is_denied() {
    let (_fixture, _control, browser) = connected_with(denied_remove).await;
    let source = directory().join("source.bin");
    tokio::fs::write(&source, vec![0x42; 1024 * 1024])
        .await
        .unwrap();
    let (upload, mut events) = transfer(
        &browser,
        Direction::Upload {
            local: source,
            remote: "/cancelled.bin".into(),
        },
    );
    assert!(matches!(
        events.recv().await,
        Some(TransferEvent::Progress { .. })
    ));
    upload.cancel_and_wait().await.unwrap();
    assert!(
        browser
            .list("/")
            .await
            .unwrap()
            .entries
            .iter()
            .any(|entry| entry.name == "cancelled.bin")
    );
}
