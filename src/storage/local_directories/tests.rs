use super::*;
use crate::storage::{AuthMethod, Connection};
use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt, path::PathBuf};

#[test]
fn unchanged_directory_skips_disk_write_but_missing_connection_still_fails() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/background-preferences")
        .join(uuid::Uuid::new_v4().to_string());
    let mut store = Store::load(directory.join("latch.db")).unwrap();
    let connection = Connection::new(
        "Synthetic".into(),
        "example.invalid".into(),
        "tester".into(),
        AuthMethod::Password,
    );
    store.upsert(connection.clone()).unwrap();
    store
        .set_local_directory(&connection.id, &directory)
        .unwrap();
    let _lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&store.path)
        .unwrap();
    store
        .set_local_directory(&connection.id, &directory)
        .unwrap();
    assert!(
        store
            .set_local_directory(&connection.id, &directory.join("changed"))
            .is_err()
    );
    assert_eq!(
        store
            .set_local_directory("missing", &directory)
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
}
