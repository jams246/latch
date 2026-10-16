use std::{fs, io};

use crate::storage::{AuthMethod, Connection, Store, tests_persistence::path};

use super::{database, key};

fn sample() -> Connection {
    Connection::new(
        "encrypted-synthetic-server".into(),
        "synthetic.example".into(),
        "synthetic-user".into(),
        AuthMethod::Password,
    )
}

#[test]
fn invalid_configuration_is_rejected_before_creating_profile_files() {
    let path = path("invalid-save");
    let mut store = Store::load(&path).unwrap();
    let mut connection = sample();
    connection.name.clear();
    store.configuration.connections.push(connection);

    assert_eq!(store.save().unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn invalid_stored_metadata_is_rejected_on_read() {
    let path = path("invalid-read");
    let mut store = Store::load(&path).unwrap();
    store.upsert(sample()).unwrap();
    let database = database::open(&path).unwrap();
    database
        .execute("UPDATE connections SET name = ''", [])
        .unwrap();
    drop(database);

    assert_eq!(
        Store::load(path).err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn encrypted_database_hides_metadata_and_requires_its_key() {
    let path = path("ciphertext");
    let mut store = Store::load(&path).unwrap();
    store.upsert(sample()).unwrap();
    for file in [&path, &path.with_file_name("key.dpapi")] {
        let bytes = fs::read(file).unwrap();
        for text in [
            b"synthetic.example".as_slice(),
            b"encrypted-synthetic-server",
            b"synthetic-user",
        ] {
            assert!(!bytes.windows(text.len()).any(|part| part == text));
        }
    }
    let raw = rusqlite::Connection::open(&path).unwrap();
    assert!(
        raw.query_row("SELECT count(*) FROM sqlite_master", [], |row| row
            .get::<_, i64>(0))
            .is_err()
    );
    assert_eq!(
        Store::load(&path).unwrap().connections(),
        store.connections()
    );
}

#[test]
fn missing_or_modified_key_never_regenerates_or_changes_database() {
    let path = path("damaged-key");
    let mut store = Store::load(&path).unwrap();
    store.upsert(sample()).unwrap();
    let original = fs::read(&path).unwrap();
    let key = path.with_file_name("key.dpapi");
    fs::remove_file(&key).unwrap();
    assert!(Store::load(&path).is_err());
    assert!(!key.exists());
    fs::write(&key, b"corrupt protected key").unwrap();
    assert_eq!(
        Store::load(&path).err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn changed_ciphertext_and_future_schema_are_rejected() {
    let path = path("damaged-database");
    let mut store = Store::load(&path).unwrap();
    store.upsert(sample()).unwrap();
    let original = fs::read(&path).unwrap();
    let mut changed = original.clone();
    changed[100] ^= 1;
    fs::write(&path, &changed).unwrap();
    assert!(Store::load(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), changed);
    fs::write(&path, original).unwrap();
    let database = database::open(&path).unwrap();
    database.execute_batch("PRAGMA user_version=999").unwrap();
    drop(database);
    assert!(Store::load(&path).is_err());
}

#[test]
fn locked_transaction_preserves_prior_memory_and_rows() {
    let path = path("locked-transaction");
    let mut store = Store::load(&path).unwrap();
    let connection = sample();
    store.upsert(connection.clone()).unwrap();
    let database = database::open(&path).unwrap();
    database.execute_batch("BEGIN EXCLUSIVE").unwrap();
    assert!(store.delete(&connection.id).is_err());
    assert_eq!(store.connections(), &[connection]);
    database.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        Store::load(path).unwrap().connections(),
        store.connections()
    );
}

#[test]
fn protected_key_roundtrips_random_256_bits_and_rejects_another_profiles_key() {
    let first = path("first-key");
    let second = path("second-key");
    Store::load(&first).unwrap().save().unwrap();
    Store::load(&second).unwrap().save().unwrap();
    let first_key = first.with_file_name("key.dpapi");
    let second_key = second.with_file_name("key.dpapi");
    assert_eq!(key::read(&first_key).unwrap().len(), 32);
    assert!(*key::read(&first_key).unwrap() != *key::read(&second_key).unwrap());
    fs::copy(second_key, first_key).unwrap();
    assert!(Store::load(first).is_err());
}

#[test]
fn failed_row_insert_rolls_back_deletes_and_preserves_memory() {
    let path = path("rollback-insert");
    let mut store = Store::load(&path).unwrap();
    let connection = sample();
    store.upsert(connection.clone()).unwrap();
    let database = database::open(&path).unwrap();
    database.execute_batch("CREATE TRIGGER reject_insert BEFORE INSERT ON connections BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
    drop(database);
    assert!(store.delete("missing-id").is_err());
    assert_eq!(store.connections(), &[connection]);
    assert_eq!(
        Store::load(path).unwrap().connections(),
        store.connections()
    );
}

#[test]
fn rollback_journal_contains_encrypted_pages_without_connection_metadata() {
    let path = path("encrypted-journal");
    let mut store = Store::load(&path).unwrap();
    store.upsert(sample()).unwrap();
    let database = database::open(&path).unwrap();
    database
        .execute_batch("BEGIN IMMEDIATE; UPDATE connections SET name='replacement';")
        .unwrap();
    let journal = fs::read(path.with_file_name(format!(
        "{}-journal",
        path.file_name().unwrap().to_str().unwrap()
    )))
    .unwrap();
    for text in [
        b"synthetic.example".as_slice(),
        b"encrypted-synthetic-server",
        b"synthetic-user",
    ] {
        assert!(!journal.windows(text.len()).any(|part| part == text));
    }
    database.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn all_auth_methods_unicode_paths_order_and_unsigned_counts_roundtrip() {
    let path = path("all-auth-methods");
    let mut store = Store::load(&path).unwrap();
    let methods = [
        AuthMethod::Password,
        AuthMethod::Agent,
        AuthMethod::KeyboardInteractive,
        AuthMethod::KeyFile {
            path: "C:\\synthetic\\clé-密钥".into(),
        },
    ];
    let counts = [0, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX];
    for (auth, count) in methods.into_iter().zip(counts) {
        let mut connection = sample();
        connection.auth = auth;
        connection.successful_shells = count;
        store.configuration.connections.push(connection);
    }
    let id = store.connections()[0].id.clone();
    store
        .configuration
        .local_directories
        .insert(id.clone(), "C:\\synthetic\\données-目录".into());
    store.save().unwrap();
    let restored = Store::load(path).unwrap();
    assert_eq!(restored.connections(), store.connections());
    assert_eq!(restored.local_directory(&id), store.local_directory(&id));
    let exported = restored.export_json().unwrap();
    let mut imported = Store::load(super::super::tests_persistence::path("auth-import")).unwrap();
    imported
        .replace_import(Store::parse_import(&exported).unwrap())
        .unwrap();
    assert_eq!(imported.connections(), store.connections());
}
