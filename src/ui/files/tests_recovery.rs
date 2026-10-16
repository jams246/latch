use super::{
    actions::ActionResult, local, pane::Side, recovery::connection_error, tests_navigation::fixture,
};
use gpui::TestAppContext;
use latch::{sftp, ssh};
use std::{path::PathBuf, time::Duration};

#[gpui::test]
fn interrupted_remote_work_keeps_browsing_locations_and_discards_stale_results(
    cx: &mut TestAppContext,
) {
    let files = fixture(cx);
    files.update(cx, |files, cx| {
        files.local = Some(local::Directory {
            path: PathBuf::from("current"),
            entries: Vec::new(),
            drives: Vec::new(),
        });
        files.directory = Some(sftp::Directory {
            path: "/current".into(),
            entries: Vec::new(),
        });
        for path in ["previous", "current", "next"] {
            files.local_history.commit(PathBuf::from(path));
            files.remote_history.commit(format!("/{path}"));
        }
        files.local_history.commit_at(1, PathBuf::from("current"));
        files.remote_history.commit_at(1, "/current".into());
        let stale_remote = files.sender.clone();
        let stale_action = files.action_sender.clone();
        files.operation = Some(files.runtime.spawn(std::future::pending()));
        files.action_task = Some(files.runtime.spawn(std::future::pending()));
        files.action_busy = true;
        files.action_side = Some(Side::Remote);

        files.connection_lost(cx);
        assert!(files.reconnecting && files.busy);
        assert!(files.operation.is_none() && files.action_task.is_none());
        assert!(!files.actions_idle());
        assert!(stale_remote.send(Err(sftp::Error::NotConnected)).is_err());
        assert!(
            stale_action
                .send(ActionResult {
                    side: Side::Remote,
                    result: Ok(()),
                    connection_lost: false,
                })
                .is_err()
        );
        files.poll(cx);
        files.navigate("/late".into(), cx);
        files.navigate_local("late".into(), cx);
        assert!(files.operation.is_none() && files.local_operation.is_none());
        assert_eq!(files.directory.as_ref().unwrap().path, "/current");
        assert_eq!(files.local.as_ref().unwrap().path, PathBuf::from("current"));
        assert_eq!(
            files.remote_history.target_back(),
            Some((0, "/previous".into()))
        );
        assert_eq!(
            files.remote_history.target_forward(),
            Some((2, "/next".into()))
        );
        assert_eq!(
            files.local_history.target_forward(),
            Some((2, "next".into()))
        );
        assert!(
            files
                .error
                .as_ref()
                .unwrap()
                .contains("may already have changed")
        );

        files.reconnected(cx);
        assert!(!files.reconnecting && files.busy);
        assert!(files.operation.is_some());
        assert_eq!(
            files.remote_history.target_forward(),
            Some((2, "/next".into()))
        );
    });
}

#[gpui::test]
fn sftp_transport_failure_requests_one_reconnect_and_preserves_cleanup_diagnostics(
    cx: &mut TestAppContext,
) {
    let files = fixture(cx);
    files.update(cx, |files, cx| {
        let (commands, mut requests) = tokio::sync::mpsc::channel(4);
        files.commands = commands;
        files.sender.send(Err(sftp::Error::Timeout)).unwrap();
        files.poll(cx);
        let request = files.runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(1), requests.recv())
                .await
                .unwrap()
        });
        assert!(matches!(request, Some(ssh::Command::Reconnect)));
        files.poll(cx);
        assert!(requests.try_recv().is_err());
        files
            .cleanup_sender
            .send(sftp::Error::PartialRetained {
                path: "C:/synthetic/.latch-recovery.partial".into(),
                reason: "Connection interrupted".into(),
            })
            .unwrap();
        files.poll(cx);
        assert!(
            files
                .error
                .as_ref()
                .unwrap()
                .contains("C:/synthetic/.latch-recovery.partial")
        );
        assert!(files.reconnecting);
    });
}

#[gpui::test]
fn local_mutation_finishes_before_deferred_remote_refresh(cx: &mut TestAppContext) {
    let files = fixture(cx);
    files.update(cx, |files, cx| {
        files.action_busy = true;
        files.action_side = Some(Side::Local);
        files.action_task = Some(files.runtime.spawn(std::future::pending()));
        files.connection_lost(cx);
        assert!(files.action_task.is_some());
        files.reconnected(cx);
        assert!(files.reconnect_refresh && files.operation.is_none());
        files
            .action_sender
            .send(ActionResult {
                side: Side::Local,
                result: Ok(()),
                connection_lost: false,
            })
            .unwrap();
        files.poll(cx);
        files.poll(cx);
        assert!(!files.reconnect_refresh && files.operation.is_some());
    });
}

#[test]
fn only_sftp_connection_errors_trigger_transport_recovery() {
    use russh_sftp::{
        client::error::Error,
        protocol::{Status, StatusCode},
    };
    assert!(connection_error(&sftp::Error::NotConnected));
    assert!(connection_error(&sftp::Error::Remote(Error::IO(
        "closed".into()
    ))));
    assert!(!connection_error(&sftp::Error::InvalidPath));
    assert!(!connection_error(&sftp::Error::DestinationExists));
    for status_code in [StatusCode::PermissionDenied, StatusCode::NoSuchFile] {
        assert!(!connection_error(&sftp::Error::Remote(Error::Status(
            Status {
                id: 1,
                status_code,
                error_message: String::new(),
                language_tag: String::new(),
            }
        ))));
    }
}
