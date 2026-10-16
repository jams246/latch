use super::*;

#[test]
fn cancelled_download_guard_preserves_partial_when_dropped() {
    let path = crate::sftp::tests::directory().join(".latch-cancelled.part");
    std::fs::write(&path, b"Partial data").unwrap();
    let (cancel, cancelled) = watch::channel(false);
    let temporary = LocalTemporary {
        path: path.clone(),
        armed: true,
        cancel: cancelled,
    };
    cancel.send(true).unwrap();
    drop(temporary);
    assert_eq!(std::fs::read(path).unwrap(), b"Partial data");
}

#[tokio::test]
async fn aborted_download_task_removes_its_local_temporary_file() {
    let path = crate::sftp::tests::directory().join(".latch-aborted.part");
    let (_cancel, cancelled) = watch::channel(false);
    let temporary = LocalTemporary {
        path: path.clone(),
        armed: true,
        cancel: cancelled,
    };
    let (ready, receiver) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let _temporary = temporary;
        let _file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&_temporary.path)
            .await
            .unwrap();
        ready.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    receiver.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!path.exists());
}
