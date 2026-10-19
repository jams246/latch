use zeroize::Zeroizing;

use crate::storage::{AuthMethod, Connection, Store, tests_persistence::store};

fn saved(tag: &str) -> (Store, Connection) {
    let mut store = store(tag);
    let connection = Connection::new(
        "Synthetic password account".into(),
        "example.invalid".into(),
        "synthetic".into(),
        AuthMethod::Password,
    );
    store.upsert(connection.clone()).unwrap();
    store.save_password(&connection, password()).unwrap();
    (store, connection)
}

fn password() -> Zeroizing<String> {
    Zeroizing::new("synthetic-only-stored-password".into())
}

#[test]
fn password_survives_reload_encrypted_without_entering_exports() {
    let (store, connection) = saved("password-reload");
    let loaded = Store::load(&store.path).unwrap();
    assert!(loaded.saved_password(&connection) == Some(&password()));
    let exported = loaded.snapshot().export_json().unwrap();
    let database = std::fs::read(&store.path).unwrap();
    for bytes in [&exported, &database] {
        assert!(
            !bytes
                .windows(password().len())
                .any(|part| part == password().as_bytes())
        );
    }
    assert!(loaded.snapshot().into_configuration().passwords.is_empty());
    let mut imported = super::super::tests_persistence::store("password-import");
    imported
        .replace_import(Store::parse_import(&exported).unwrap())
        .unwrap();
    assert!(imported.saved_password(&connection).is_none());
}

#[test]
fn password_clear_and_delete_are_durable_and_preserve_metadata() {
    let (mut store, connection) = saved("password-clear");
    store.clear_password(&connection.id).unwrap();
    assert!(
        Store::load(&store.path)
            .unwrap()
            .saved_password(&connection)
            .is_none()
    );
    assert_eq!(store.connections(), std::slice::from_ref(&connection));
    store.save_password(&connection, password()).unwrap();
    store.delete(&connection.id).unwrap();
    assert!(
        Store::load(&store.path)
            .unwrap()
            .configuration
            .passwords
            .is_empty()
    );
}

#[test]
fn name_changes_preserve_password_but_account_changes_invalidate_it() {
    for change in 0..4 {
        let (mut store, original) = saved("password-account");
        let mut changed = original.clone();
        changed.name = "Renamed".into();
        store.upsert(changed.clone()).unwrap();
        assert!(store.saved_password(&changed).is_some());
        match change {
            0 => changed.host = "other.invalid".into(),
            1 => changed.port = 2222,
            2 => changed.username = "other-user".into(),
            _ => changed.auth = AuthMethod::Agent,
        }
        assert!(store.saved_password(&changed).is_none());
        store.upsert(changed.clone()).unwrap();
        store.save_password(&original, password()).unwrap();
        assert!(
            Store::load(&store.path)
                .unwrap()
                .saved_password(&changed)
                .is_none()
        );
    }
}

#[test]
fn deleted_connections_and_non_password_methods_cannot_save_secrets() {
    let (mut store, connection) = saved("password-deleted");
    store.delete(&connection.id).unwrap();
    store.save_password(&connection, password()).unwrap();
    let mut connection = connection;
    connection.auth = AuthMethod::KeyboardInteractive;
    store.upsert(connection.clone()).unwrap();
    store.save_password(&connection, password()).unwrap();
    assert!(store.configuration.passwords.is_empty());
}

#[test]
fn replacing_metadata_import_removes_saved_passwords() {
    let (mut store, connection) = saved("password-import-replace");
    let backup = store.snapshot().export_json().unwrap();
    store
        .replace_import(Store::parse_import(&backup).unwrap())
        .unwrap();
    assert!(
        Store::load(&store.path)
            .unwrap()
            .saved_password(&connection)
            .is_none()
    );
}
