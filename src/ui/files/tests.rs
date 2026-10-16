use super::{TransferEvent, polling::transfer_status};

#[test]
fn retained_terminal_states_release_transfer_controls_but_progress_does_not() {
    assert_eq!(
        transfer_status(TransferEvent::Completed),
        ("Transfer complete".into(), true)
    );
    assert_eq!(
        transfer_status(TransferEvent::Cancelled),
        ("Transfer cancelled".into(), true)
    );
    assert_eq!(
        transfer_status(TransferEvent::Failed("failure".into())),
        ("failure Refresh folder to reconnect.".into(), true)
    );
    assert!(
        !transfer_status(TransferEvent::Progress {
            completed: 1,
            total: Some(2)
        })
        .1
    );
}
