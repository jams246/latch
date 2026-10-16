use std::time::Duration;

use tokio::sync::{mpsc, watch};

use super::{
    Direction, TransferEvent,
    tests::{connected, directory, finished},
    transfer_progress::Reporter,
};

#[test]
fn aggregate_progress_retains_latest_filename_and_bytes_when_event_queue_is_full() {
    let (events, mut queued) = mpsc::channel(1);
    let (latest, retained) = watch::channel(None);
    let mut progress = Reporter::new(events, latest);
    progress.total(Some(9));
    progress.file("first.txt".into(), Some(3));
    progress.advance(3);
    progress.file("second.txt".into(), Some(6));
    progress.advance(6);
    assert_eq!(
        *retained.borrow(),
        Some(TransferEvent::Progress {
            name: "second.txt".into(),
            completed: 9,
            total: Some(9),
        })
    );
    assert!(matches!(
        queued.try_recv(),
        Ok(TransferEvent::Progress { completed: 0, .. })
    ));
}

#[test]
fn unknown_aggregate_total_is_not_replaced_by_current_file_length() {
    let (events, _queued) = mpsc::channel(1);
    let (latest, retained) = watch::channel(None);
    let mut progress = Reporter::new(events, latest);
    progress.total(None);
    progress.file("known.txt".into(), Some(3));
    progress.advance(3);
    assert!(matches!(
        *retained.borrow(),
        Some(TransferEvent::Progress { total: None, .. })
    ));
}

#[tokio::test]
async fn folder_upload_and_download_report_complete_tree_totals_and_current_filenames() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory();
    tokio::fs::create_dir_all(local.join("source/nested/empty"))
        .await
        .unwrap();
    tokio::fs::write(local.join("source/first.txt"), b"abc")
        .await
        .unwrap();
    tokio::fs::write(local.join("source/nested/second.txt"), b"123456")
        .await
        .unwrap();
    let (events, mut incoming) = mpsc::channel(32);
    let upload = browser.transfer_many(
        vec![Direction::Upload {
            local: local.join("source"),
            remote: "/progress-folder".into(),
        }],
        events,
    );
    tokio::time::timeout(Duration::from_secs(15), assert_tree_progress(&mut incoming))
        .await
        .unwrap();
    assert!(matches!(
        upload.progress_event(),
        Some(TransferEvent::Progress {
            completed: 9,
            total: Some(9),
            ..
        })
    ));
    let (events, mut incoming) = mpsc::channel(32);
    let download = browser.transfer_many(
        vec![Direction::Download {
            remote: "/progress-folder".into(),
            local: local.join("downloaded"),
        }],
        events,
    );
    tokio::time::timeout(Duration::from_secs(15), assert_tree_progress(&mut incoming))
        .await
        .unwrap();
    assert!(matches!(
        download.progress_event(),
        Some(TransferEvent::Progress {
            completed: 9,
            total: Some(9),
            ..
        })
    ));
    upload.cancel_and_wait().await.unwrap();
    download.cancel_and_wait().await.unwrap();
}

async fn assert_tree_progress(incoming: &mut mpsc::Receiver<TransferEvent>) {
    let mut last = 0;
    let mut names = std::collections::BTreeSet::new();
    loop {
        match incoming.recv().await.unwrap() {
            TransferEvent::Progress {
                name,
                completed,
                total,
            } => {
                assert_eq!(total, Some(9));
                assert!(completed >= last);
                last = completed;
                names.insert(name);
            }
            event => {
                assert_eq!(event, TransferEvent::Completed);
                break;
            }
        }
    }
    assert_eq!(last, 9);
    assert_eq!(
        names,
        ["first.txt".to_owned(), "second.txt".to_owned()].into()
    );
}

#[tokio::test]
async fn empty_folder_copy_completes_without_inventing_file_progress() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory();
    let (events, mut incoming) = mpsc::channel(32);
    let transfer = browser.transfer_many(
        vec![Direction::Upload {
            local,
            remote: "/empty-progress".into(),
        }],
        events,
    );
    assert_eq!(finished(&mut incoming).await, TransferEvent::Completed);
    assert!(transfer.progress_event().is_none());
    transfer.cancel_and_wait().await.unwrap();
}
