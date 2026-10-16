use super::{Files, LocalResult, RemoteResult, local, pane::Side};
use gpui::{AppContext, Entity, TestAppContext};
use latch::{sftp, ssh};
use std::{
    io,
    path::PathBuf,
    sync::{Arc, mpsc},
};

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/dual-pane/ui-state")
        .join(uuid::Uuid::new_v4().to_string())
}

fn local_directory(path: &str) -> local::Directory {
    local::Directory {
        path: path.into(),
        entries: Vec::new(),
        drives: Vec::new(),
    }
}

pub(super) fn fixture(cx: &mut TestAppContext) -> Entity<Files> {
    let (commands, _) = tokio::sync::mpsc::channel(1);
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let config = directory();
    let initial = config.join("initial-local");
    let connection = super::tests::connection();
    std::fs::create_dir_all(&initial).unwrap();
    let files = cx.new(|cx| {
        Files::new(
            &connection,
            commands,
            runtime,
            Some(initial),
            "synthetic-tab".into(),
            mpsc::channel().0,
            cx,
        )
    });
    files.update(cx, |files, _| {
        for operation in [files.operation.take(), files.local_operation.take()]
            .into_iter()
            .flatten()
        {
            operation.abort();
        }
        (files.sender, files.events) = mpsc::channel::<Result<RemoteResult, sftp::Error>>();
        (files.local_sender, files.local_events) = mpsc::channel::<LocalResult>();
        files.busy = false;
        files.local_busy = false;
    });
    files
}

#[gpui::test]
fn failed_navigation_keeps_both_directories_and_history_positions(cx: &mut TestAppContext) {
    let files = fixture(cx);
    files.update(cx, |files, cx| {
        files.local = Some(local_directory("current"));
        files.directory = Some(sftp::Directory {
            path: "/current".into(),
            entries: Vec::new(),
        });
        for path in ["previous", "current"] {
            files.local_history.commit(PathBuf::from(path));
            files.remote_history.commit(format!("/{path}"));
        }
        files.pending_local_history = Some(0);
        files.pending_remote_history = Some(0);
        files.local_busy = true;
        files.busy = true;
        files
            .local_sender
            .send(Err(io::Error::from(io::ErrorKind::NotFound)))
            .unwrap();
        files.sender.send(Err(sftp::Error::InvalidPath)).unwrap();

        files.poll(cx);

        assert_eq!(files.local.as_ref().unwrap().path, PathBuf::from("current"));
        assert_eq!(files.directory.as_ref().unwrap().path, "/current");
        assert_eq!(
            files.local_history.target_back(),
            Some((0, PathBuf::from("previous")))
        );
        assert_eq!(
            files.remote_history.target_back(),
            Some((0, "/previous".into()))
        );
        assert!(!files.local_history.can_forward() && !files.remote_history.can_forward());
        assert!(!files.local_busy && !files.busy);
        assert!(files.pending_local_history.is_none() && files.pending_remote_history.is_none());
    });
}

#[gpui::test]
fn successful_history_navigation_and_refresh_do_not_duplicate_paths(cx: &mut TestAppContext) {
    let files = fixture(cx);
    files.update(cx, |files, cx| {
        files.local_history.commit(PathBuf::from("first"));
        files.local_history.commit(PathBuf::from("second"));
        files.pending_local_history = Some(0);
        files
            .local_sender
            .send(Ok(local_directory("first")))
            .unwrap();
        files.poll(cx);
        assert!(!files.local_history.can_back());
        assert_eq!(
            files.local_history.target_forward(),
            Some((1, PathBuf::from("second")))
        );

        files
            .local_sender
            .send(Ok(local_directory("first")))
            .unwrap();
        files.poll(cx);
        assert!(!files.local_history.can_back());
        assert_eq!(
            files.local_history.target_forward(),
            Some((1, PathBuf::from("second")))
        );

        files.pending_local_history = Some(1);
        files
            .local_sender
            .send(Ok(local_directory("second")))
            .unwrap();
        files.poll(cx);
        assert_eq!(
            files.local_history.target_back(),
            Some((0, PathBuf::from("first")))
        );
        assert!(!files.local_history.can_forward());
        assert_eq!(files.local.as_ref().unwrap().path, PathBuf::from("second"));
    });
}

#[gpui::test]
fn closing_ignores_late_directory_results_errors_and_navigation(cx: &mut TestAppContext) {
    let files = fixture(cx);
    files.update(cx, |files, cx| {
        files.local = Some(local_directory("current"));
        files.local_history.commit(PathBuf::from("current"));
        files.directory = Some(sftp::Directory {
            path: "/current".into(),
            entries: Vec::new(),
        });
        files.remote_history.commit("/current".into());
        files.prepare_close(cx);
        files
            .local_sender
            .send(Ok(local_directory("late")))
            .unwrap();
        files.sender.send(Err(sftp::Error::NotConnected)).unwrap();

        files.poll(cx);
        files.navigate("/late".into(), cx);
        files.navigate_local(PathBuf::from("late"), cx);
        files.history_back(Side::Local, cx);
        files.history_forward(Side::Remote, cx);

        assert_eq!(files.local.as_ref().unwrap().path, PathBuf::from("current"));
        assert_eq!(files.directory.as_ref().unwrap().path, "/current");
        assert!(!files.local_history.can_back() && !files.remote_history.can_back());
        assert!(files.operation.is_none() && files.local_operation.is_none());
        assert!(files.closing);
    });
}

#[gpui::test]
fn new_remote_browser_starts_at_server_default_despite_remembered_local_path(
    cx: &mut TestAppContext,
) {
    let config = directory();
    let remembered = config.join("remembered-local");
    let connection = super::tests::connection();
    std::fs::create_dir_all(&remembered).unwrap();
    let (commands, _) = tokio::sync::mpsc::channel::<ssh::Command>(1);
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let files = cx.new(|cx| {
        Files::new(
            &connection,
            commands,
            runtime,
            Some(remembered),
            "synthetic-tab".into(),
            mpsc::channel().0,
            cx,
        )
    });

    files.read_with(cx, |files, cx| {
        assert_eq!(files.connection_name, connection.name);
        assert_eq!(files.connection_id, connection.id);
        assert_eq!(files.path.read(cx).value.as_str(), ".");
        assert!(files.directory.is_none());
        assert!(!files.remote_history.can_back() && !files.remote_history.can_forward());
        assert!(files.busy);
    });
}
