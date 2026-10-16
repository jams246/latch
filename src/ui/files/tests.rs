use super::{TransferEvent, polling::transfer_status};

pub(super) fn connection() -> latch::storage::Connection {
    let mut connection = latch::storage::Connection::new(
        "Synthetic SFTP server".into(),
        "example.invalid".into(),
        "synthetic".into(),
        latch::storage::AuthMethod::Agent,
    );
    connection.id = uuid::Uuid::nil().to_string();
    connection
}

#[test]
fn retained_terminal_states_release_transfer_controls_but_progress_does_not() {
    assert_eq!(
        transfer_status(TransferEvent::Completed),
        ("Transfer complete".into(), true)
    );
    assert_eq!(
        transfer_status(TransferEvent::Cancelled),
        (String::new(), true)
    );
    assert_eq!(
        transfer_status(TransferEvent::Failed("failure".into())),
        ("failure Refresh folder to reconnect.".into(), true)
    );
    assert!(
        !transfer_status(TransferEvent::Progress {
            name: "file.txt".into(),
            completed: 1,
            total: Some(2)
        })
        .1
    );
}

#[gpui::test]
fn cancellation_refreshes_both_panes_without_posting_notices(cx: &mut gpui::TestAppContext) {
    let (handle, mut visual) = super::tests_interaction::fixture(cx);
    handle
        .update(&mut visual, |files, _, cx| {
            let (events, progress) = tokio::sync::mpsc::channel(1);
            events.try_send(TransferEvent::Cancelled).unwrap();
            files.progress = Some(progress);
            files.batch_copy = true;
            files.action_status = [
                Some("Old local notice".into()),
                Some("Old remote notice".into()),
            ];
            files.poll(cx);
            assert_eq!(files.action_status, [None, None]);
            assert!(files.local_busy && files.busy);
            assert!(files.progress.is_none() && files.metrics.is_none());
        })
        .unwrap();
}
