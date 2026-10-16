use super::*;

#[tokio::test]
async fn replacement_capture_and_publication_pause_then_cancel_before_changing_destinations() {
    let (_fixture, _control, browser) = connected().await;
    let local = directory().join("original.txt");
    let staged = local.with_file_name("staged.txt");
    tokio::fs::write(&local, b"Original local").await.unwrap();
    tokio::fs::write(&staged, b"Staged local").await.unwrap();
    let mut remote_file = browser.session.create("/staged.txt").await.unwrap();
    remote_file.write_all(b"Staged remote").await.unwrap();
    remote_file.close().await.unwrap();
    for (direction, staged) in [
        (
            Direction::Upload {
                local: staged.clone(),
                remote: "/hello.txt".into(),
            },
            Direction::Upload {
                local: staged.clone(),
                remote: "/staged.txt".into(),
            },
        ),
        (
            Direction::Download {
                remote: "/hello.txt".into(),
                local: local.clone(),
            },
            Direction::Download {
                remote: "/hello.txt".into(),
                local: staged.clone(),
            },
        ),
    ] {
        let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
        let (_pause, mut paused) = tokio::sync::watch::channel(true);
        let capturing =
            capture_controlled(&browser.session, &direction, &mut cancelled, &mut paused);
        tokio::pin!(capturing);
        tokio::select! {
            _ = &mut capturing => panic!("Capture ran while paused"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => (),
        }
        cancel.send(true).unwrap();
        assert!(matches!(capturing.await, Err(Error::Cancelled)));

        let replacement = capture(&browser.session, &direction).await.unwrap();
        let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
        let (_pause, mut paused) = tokio::sync::watch::channel(true);
        let publishing = publish_controlled(
            &browser.session,
            &staged,
            &replacement,
            &mut cancelled,
            &mut paused,
        );
        tokio::pin!(publishing);
        tokio::select! {
            _ = &mut publishing => panic!("Publication ran while paused"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => (),
        }
        cancel.send(true).unwrap();
        assert!(matches!(publishing.await, Err(Error::Cancelled)));
    }
    assert_eq!(
        browser.session.read("/hello.txt").await.unwrap(),
        b"Synthetic SFTP content\n"
    );
    assert_eq!(tokio::fs::read(local).await.unwrap(), b"Original local");
    assert_eq!(tokio::fs::read(staged).await.unwrap(), b"Staged local");
}
