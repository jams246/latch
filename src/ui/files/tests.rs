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
        ("Transfer cancelled".into(), true)
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
