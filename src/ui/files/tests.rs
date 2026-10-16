use super::{TransferEvent, polling::transfer_finished};

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
    let mut error = None;
    assert!(transfer_finished(TransferEvent::Completed, &mut error));
    assert!(error.is_none());
    assert!(transfer_finished(TransferEvent::Cancelled, &mut error));
    assert!(transfer_finished(
        TransferEvent::Failed("failure".into()),
        &mut error
    ));
    assert!(error.as_ref().unwrap().contains("failure"));
    assert!(!transfer_finished(
        TransferEvent::Progress {
            name: "file.txt".into(),
            completed: 1,
            total: Some(2)
        },
        &mut error
    ));
}

#[gpui::test]
fn terminal_events_refresh_both_panes_without_notification_rows(cx: &mut gpui::TestAppContext) {
    for event in [
        TransferEvent::Completed,
        TransferEvent::Cancelled,
        TransferEvent::Failed("synthetic failure".into()),
    ] {
        let (handle, mut visual) = super::tests_interaction::fixture(cx);
        handle
            .update(&mut visual, |files, _, cx| {
                let (events, progress) = tokio::sync::mpsc::channel(1);
                events.try_send(event).unwrap();
                files.progress = Some(progress);
                files.poll(cx);
                assert!(files.local_busy && files.busy);
                assert!(files.progress.is_none() && files.metrics.is_none());
            })
            .unwrap();
        super::tests_interaction::draw(handle, &mut visual);
        assert!(visual.debug_bounds("local-status").is_none());
        assert!(visual.debug_bounds("remote-status").is_none());
        assert!(visual.debug_bounds("operations-bar").is_none());
    }
}
