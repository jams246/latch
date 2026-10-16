use super::{tests::fixture, *};

#[test]
fn remembered_local_directory_survives_restart_and_can_be_replaced() {
    let configuration = fixture("preferences");
    let first = fixture("first");
    let second = fixture("second");
    remember(&configuration, "connection", &first).unwrap();
    assert_eq!(initial_path(&configuration, "connection"), first);
    remember(&configuration, "connection", &second).unwrap();
    assert_eq!(initial_path(&configuration, "connection"), second);
}

#[test]
fn connections_remember_independent_local_directories() {
    let configuration = fixture("connection-preferences");
    let first = fixture("connection-first");
    let second = fixture("connection-second");
    remember(&configuration, "first-connection", &first).unwrap();
    remember(&configuration, "second-connection", &second).unwrap();
    assert_eq!(initial_path(&configuration, "first-connection"), first);
    assert_eq!(initial_path(&configuration, "second-connection"), second);
}

#[test]
fn renaming_a_connection_preserves_its_remembered_directory() {
    let configuration = fixture("renamed-preferences");
    let directory = fixture("renamed-directory");
    let mut connection = latch::storage::Connection::new(
        "Original name".into(),
        "example.test".into(),
        "synthetic".into(),
        latch::storage::AuthMethod::Password,
    );
    remember(&configuration, &connection.id, &directory).unwrap();
    connection.name = "Renamed connection".into();
    assert_eq!(initial_path(&configuration, &connection.id), directory);
}

#[test]
fn missing_preferences_do_not_reuse_another_connection_or_legacy_directory() {
    let configuration = fixture("missing-preferences");
    let directory = fixture("legacy-directory");
    fs::write(
        configuration.join("sftp-local-directory.json"),
        serde_json::to_vec(&directory).unwrap(),
    )
    .unwrap();
    remember(&configuration, "other-connection", &directory).unwrap();
    let selected = initial_path(&configuration, "new-connection");
    assert_ne!(selected, directory);
    assert!(selected.is_dir());
}

#[test]
fn unavailable_remembered_directory_falls_back_to_an_existing_directory() {
    let configuration = fixture("unavailable");
    let unavailable = configuration.join("missing");
    remember(&configuration, "connection", &unavailable).unwrap();
    let selected = initial_path(&configuration, "connection");
    assert_ne!(selected, unavailable);
    assert!(selected.is_dir());
}

#[test]
fn deleting_a_remembered_directory_uses_the_default_directory() {
    let configuration = fixture("deleted-preferences");
    let directory = fixture("deleted-directory");
    remember(&configuration, "connection", &directory).unwrap();
    fs::remove_dir(&directory).unwrap();
    let selected = initial_path(&configuration, "connection");
    assert_ne!(selected, directory);
    assert!(selected.is_dir());
}

#[test]
fn arbitrary_connection_ids_stay_inside_the_preferences_directory() {
    let configuration = fixture("safe-identity");
    let directory = fixture("safe-directory");
    let connection_id = "../outside\\file:server";
    remember(&configuration, connection_id, &directory).unwrap();
    let destination = preference_path(&configuration, connection_id);
    assert_eq!(
        destination.parent(),
        Some(configuration.join("sftp-local-directories").as_path())
    );
    assert_eq!(initial_path(&configuration, connection_id), directory);
}

#[test]
fn invalid_preferences_fall_back_without_changing_the_original_file() {
    let configuration = fixture("invalid-preferences");
    let directory = fixture("valid-directory");
    remember(&configuration, "connection", &directory).unwrap();
    let destination = preference_path(&configuration, "connection");
    fs::write(&destination, b"invalid JSON").unwrap();
    assert!(initial_path(&configuration, "connection").is_dir());
    assert_eq!(fs::read(destination).unwrap(), b"invalid JSON");
}
