use super::{database, write};
use crate::storage::{AuthMethod, Connection, Store, tests_persistence::path};

#[test]
fn version_one_database_migrates_without_losing_metadata_or_changing_key() {
    let path = path("password-migration");
    let mut store = Store::load(&path).unwrap();
    let connection = Connection::new(
        "Synthetic".into(),
        "example.invalid".into(),
        "synthetic".into(),
        AuthMethod::Password,
    );
    store.upsert(connection.clone()).unwrap();
    let wrapped_key = std::fs::read(path.with_file_name("key.dpapi")).unwrap();
    let database = database::open(&path).unwrap();
    database
        .execute_batch("DROP TABLE passwords; PRAGMA user_version=1;")
        .unwrap();
    drop(database);
    let mut restored = Store::load(&path).unwrap();
    assert_eq!(restored.connections(), store.connections());
    assert!(restored.saved_password(&connection).is_none());
    restored
        .save_password(
            &connection,
            zeroize::Zeroizing::new("synthetic-only".into()),
        )
        .unwrap();
    assert!(
        Store::load(&path)
            .unwrap()
            .saved_password(&connection)
            .is_some()
    );
    assert_eq!(
        std::fs::read(path.with_file_name("key.dpapi")).unwrap(),
        wrapped_key
    );
}

#[test]
fn cipher_secure_delete_is_enabled_for_cleared_password_pages() {
    let path = path("password-secure-delete");
    write(&path, &Default::default()).unwrap();
    let database = database::open(&path).unwrap();
    let secure_delete: i64 = database
        .query_row("PRAGMA secure_delete", [], |row| row.get(0))
        .unwrap();
    assert_eq!(secure_delete, 1);
}

#[test]
fn failed_clear_preserves_saved_password_in_memory_and_database() {
    let path = path("password-clear-rollback");
    let mut store = Store::load(&path).unwrap();
    let connection = Connection::new(
        "Synthetic".into(),
        "example.invalid".into(),
        "synthetic".into(),
        AuthMethod::Password,
    );
    store.upsert(connection.clone()).unwrap();
    store
        .save_password(
            &connection,
            zeroize::Zeroizing::new("synthetic-only".into()),
        )
        .unwrap();
    let database = database::open(&path).unwrap();
    database.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    assert!(store.clear_password(&connection.id).is_err());
    assert!(store.saved_password(&connection).is_some());
    database.execute_batch("ROLLBACK;").unwrap();
    assert!(
        Store::load(path)
            .unwrap()
            .saved_password(&connection)
            .is_some()
    );
}
