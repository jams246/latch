use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

#[tokio::test(start_paused = true)]
async fn shutdown_deadline_aborts_task_and_reports_possible_partial_data() {
    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let cleanup = Dropped(dropped.clone());
    let task = tokio::spawn(async move {
        let _cleanup = cleanup;
        std::future::pending::<Result<(), Error>>().await
    });
    tokio::task::yield_now().await;
    let (cancel, _cancelled) = watch::channel(false);
    let (paused, _pause_requested) = watch::channel(false);
    let (_terminal, terminal) = watch::channel(None);
    let (_progress, progress) = watch::channel(None);
    let (_resolver, conflict, resolutions) = Resolver::channel();
    let transfer = Transfer {
        cancel,
        paused,
        task: Some(task),
        terminal,
        progress,
        conflict,
        resolutions,
    };
    let error = transfer.cancel_and_wait().await.unwrap_err();
    assert!(matches!(error, Error::CleanupTimeout));
    assert!(
        error
            .to_string()
            .contains("temporary .latch file may remain")
    );
    assert!(dropped.load(Ordering::SeqCst));
}
