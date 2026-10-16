use tokio::{io::AsyncWriteExt, sync::mpsc};

use super::{
    Direction, Error, TransferEvent,
    tests::{connected, directory},
    transfer_files::{download, upload},
    transfer_progress::Reporter,
    transfer_publish::{Replacement, capture as capture_controlled, publish as publish_controlled},
};

async fn capture(
    session: &russh_sftp::client::SftpSession,
    direction: &Direction,
) -> Result<Replacement, Error> {
    let (_cancel, mut cancelled) = tokio::sync::watch::channel(false);
    let (_pause, mut paused) = tokio::sync::watch::channel(false);
    capture_controlled(session, direction, &mut cancelled, &mut paused).await
}

async fn publish(
    session: &russh_sftp::client::SftpSession,
    staged: &Direction,
    replacement: &Replacement,
) -> Result<(), Error> {
    let (_cancel, mut cancelled) = tokio::sync::watch::channel(false);
    let (_pause, mut paused) = tokio::sync::watch::channel(false);
    publish_controlled(session, staged, replacement, &mut cancelled, &mut paused).await
}

fn reporter() -> (Reporter, mpsc::Receiver<TransferEvent>) {
    let (events, incoming) = mpsc::channel(32);
    let (latest, _receiver) = tokio::sync::watch::channel(None);
    (Reporter::new(events, latest), incoming)
}

async fn stop_after_bytes(
    events: &mut mpsc::Receiver<TransferEvent>,
    cancel: tokio::sync::watch::Sender<bool>,
) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(event) = events.recv().await {
            if matches!(event, TransferEvent::Progress { completed, .. } if completed > 0) {
                cancel.send(true).unwrap();
                return;
            }
        }
        panic!("Transfer stopped before copying bytes");
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn approved_file_replacements_publish_complete_uploads_and_downloads() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("replace.txt");
    tokio::fs::write(&local, b"Replacement content")
        .await
        .unwrap();
    let upload_direction = Direction::Upload {
        local: local.clone(),
        remote: "/hello.txt".into(),
    };
    let replacement = capture(&browser.session, &upload_direction).await.unwrap();
    let (_cancel, mut cancelled) = tokio::sync::watch::channel(false);
    let (_pause, mut paused) = tokio::sync::watch::channel(false);
    let (mut progress, _events) = reporter();
    upload(
        &browser.session,
        &local,
        "/hello.txt",
        &mut cancelled,
        &mut paused,
        &mut progress,
        Some(&replacement),
    )
    .await
    .unwrap();
    assert_eq!(
        browser.session.read("/hello.txt").await.unwrap(),
        b"Replacement content"
    );
    let direction = Direction::Download {
        remote: "/hello.txt".into(),
        local: local.clone(),
    };
    tokio::fs::write(&local, b"Old download content")
        .await
        .unwrap();
    let replacement = capture(&browser.session, &direction).await.unwrap();
    download(
        &browser.session,
        "/hello.txt",
        &local,
        &mut cancelled,
        &mut paused,
        &mut progress,
        Some(&replacement),
    )
    .await
    .unwrap();
    assert_eq!(
        tokio::fs::read(local).await.unwrap(),
        b"Replacement content"
    );
}

#[tokio::test]
async fn cancelling_replacement_upload_preserves_original_and_partial_sibling() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("replace.bin");
    tokio::fs::write(&local, vec![0x42; 8 * 1024 * 1024])
        .await
        .unwrap();
    let direction = Direction::Upload {
        local: local.clone(),
        remote: "/hello.txt".into(),
    };
    let replacement = capture(&browser.session, &direction).await.unwrap();
    let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
    let (_pause, mut paused) = tokio::sync::watch::channel(false);
    let (mut progress, mut events) = reporter();
    let (result, ()) = tokio::join!(
        upload(
            &browser.session,
            &local,
            "/hello.txt",
            &mut cancelled,
            &mut paused,
            &mut progress,
            Some(&replacement)
        ),
        stop_after_bytes(&mut events, cancel)
    );
    let Error::PartialRetained { path, .. } = result.unwrap_err() else {
        panic!("Missing partial path")
    };
    assert_eq!(
        browser.session.read("/hello.txt").await.unwrap(),
        b"Synthetic SFTP content\n"
    );
    let partial = browser.session.read(path).await.unwrap();
    assert!(!partial.is_empty() && partial.len() < 8 * 1024 * 1024);
}

#[tokio::test]
async fn cancelling_replacement_download_preserves_original_and_partial_sibling() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("replace.bin");
    tokio::fs::write(&local, b"Original data").await.unwrap();
    let direction = Direction::Download {
        remote: "/large.bin".into(),
        local: local.clone(),
    };
    let replacement = capture(&browser.session, &direction).await.unwrap();
    let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
    let (_pause, mut paused) = tokio::sync::watch::channel(false);
    let (mut progress, mut events) = reporter();
    let (result, ()) = tokio::join!(
        download(
            &browser.session,
            "/large.bin",
            &local,
            &mut cancelled,
            &mut paused,
            &mut progress,
            Some(&replacement)
        ),
        stop_after_bytes(&mut events, cancel)
    );
    let Error::PartialRetained { path, .. } = result.unwrap_err() else {
        panic!("Missing partial path")
    };
    assert_eq!(tokio::fs::read(local).await.unwrap(), b"Original data");
    let partial = tokio::fs::read(path).await.unwrap();
    assert!(!partial.is_empty() && partial.len() < 16 * 1024 * 1024);
}

#[tokio::test]
async fn replacement_snapshot_rejects_changed_local_file_and_remote_tree() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("original.txt");
    tokio::fs::write(&local, b"Old").await.unwrap();
    let direction = Direction::Download {
        remote: "/hello.txt".into(),
        local: local.clone(),
    };
    let replacement = capture(&browser.session, &direction).await.unwrap();
    tokio::fs::write(&local, b"Concurrent local data")
        .await
        .unwrap();
    let staged = local.with_file_name("staged.txt");
    tokio::fs::write(&staged, b"Staged").await.unwrap();
    let staged = Direction::Download {
        remote: "/hello.txt".into(),
        local: staged,
    };
    assert!(matches!(
        publish(&browser.session, &staged, &replacement).await,
        Err(Error::DestinationExists)
    ));
    assert_eq!(
        tokio::fs::read(local).await.unwrap(),
        b"Concurrent local data"
    );
    let direction = Direction::Upload {
        local: directory(),
        remote: "/folder".into(),
    };
    let replacement = capture(&browser.session, &direction).await.unwrap();
    let mut file = browser.session.create("/folder/nested.txt").await.unwrap();
    file.write_all(b"Changed remote child with a different length")
        .await
        .unwrap();
    file.close().await.unwrap();
    browser.session.create_dir("/staged").await.unwrap();
    let staged = Direction::Upload {
        local: directory(),
        remote: "/staged".into(),
    };
    assert!(matches!(
        publish(&browser.session, &staged, &replacement).await,
        Err(Error::DestinationExists)
    ));
    assert_eq!(
        browser.session.read("/folder/nested.txt").await.unwrap(),
        b"Changed remote child with a different length"
    );
}

mod controls;
mod interactive;
mod trees;
