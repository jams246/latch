use std::time::Duration;

use tokio::sync::mpsc;

use super::{
    Direction, Transfer, TransferEvent,
    tests::{connected, connected_with, directory},
};

async fn retained(transfer: &Transfer) -> TransferEvent {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(event) = transfer.terminal_event() {
                return event;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}

async fn pause_receiver(events: &mut mpsc::Receiver<TransferEvent>) {
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(matches!(
        events.try_recv(),
        Ok(TransferEvent::Progress { .. })
    ));
}

#[tokio::test]
async fn slow_ui_retains_success_after_terminal_event_delivery_times_out() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("download.bin");
    let (events, mut progress) = mpsc::channel(1);
    let transfer = browser.transfer(
        Direction::Download {
            remote: "/hello.txt".into(),
            local,
        },
        events,
    );
    assert_eq!(retained(&transfer).await, TransferEvent::Completed);
    pause_receiver(&mut progress).await;
    assert_eq!(transfer.terminal_event(), Some(TransferEvent::Completed));
    transfer.cancel_and_wait().await.unwrap();
}

#[tokio::test]
async fn slow_ui_retains_cancellation_after_terminal_event_delivery_times_out() {
    let (_fixture, _control, browser) = connected().await;
    let (events, mut progress) = mpsc::channel(1);
    let transfer = browser.transfer(
        Direction::Download {
            remote: "/large.bin".into(),
            local: directory().join("cancelled.bin"),
        },
        events,
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while progress.is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    transfer.cancel();
    assert_eq!(retained(&transfer).await, TransferEvent::Cancelled);
    pause_receiver(&mut progress).await;
    assert_eq!(transfer.terminal_event(), Some(TransferEvent::Cancelled));
    transfer.cancel_and_wait().await.unwrap();
}

fn slow_close(stream: russh::ChannelStream<russh::server::Msg>) {
    tokio::spawn(super::tests_server::serve_faulty(stream, true, false));
}

#[tokio::test]
async fn slow_ui_retains_failure_after_terminal_event_delivery_times_out() {
    let (_fixture, _control, browser) = connected_with(slow_close).await;
    browser.session.set_timeout(1);
    let (events, mut progress) = mpsc::channel(1);
    let transfer = browser.transfer(
        Direction::Download {
            remote: "/hello.txt".into(),
            local: directory().join("failure.bin"),
        },
        events,
    );
    assert!(
        matches!(retained(&transfer).await, TransferEvent::Failed(message) if message.contains("could not be closed"))
    );
    pause_receiver(&mut progress).await;
    assert!(matches!(
        transfer.terminal_event(),
        Some(TransferEvent::Failed(_))
    ));
    assert!(transfer.cancel_and_wait().await.is_err());
}
