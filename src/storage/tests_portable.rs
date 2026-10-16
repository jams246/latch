use std::{fs, io, path::PathBuf};

use super::{AuthMethod, Connection, SortMode, Store, tests_persistence::path};

fn directory(tag: &str) -> PathBuf {
    path(tag).parent().unwrap().to_path_buf()
}

fn legacy_store(directory: &std::path::Path, name: &str) -> Store {
    let mut store = Store::load(directory.join("connections.json")).unwrap();
    let connection = Connection::new(
        name.into(),
        "127.0.0.1".into(),
        "synthetic-user".into(),
        AuthMethod::Password,
    );
    store.upsert(connection.clone()).unwrap();
    store.record_successful_shell(&connection.id).unwrap();
    store.set_sort_mode(SortMode::FrequentAndName).unwrap();
    store
}

#[test]
fn roaming_configuration_and_host_trust_migrate_without_changing_originals() {
    let roaming = directory("roaming-source");
    let portable = directory("portable-destination");
    let legacy = legacy_store(&roaming, "Roaming server");
    fs::write(roaming.join("known_hosts"), b"synthetic known host").unwrap();
    let original = fs::read(&legacy.path).unwrap();

    let migrated = Store::load_portable(&portable, Some(&roaming)).unwrap();

    assert_eq!(migrated.path, portable.join("latch.json"));
    assert_eq!(migrated.connections()[0].name, "Roaming server");
    assert_eq!(migrated.connections()[0].successful_shells, 1);
    assert_eq!(migrated.sort_mode(), SortMode::FrequentAndName);
    assert_eq!(fs::read(&legacy.path).unwrap(), original);
    assert_eq!(fs::read(migrated.path).unwrap(), original);
    assert_eq!(
        fs::read(portable.join("known_hosts")).unwrap(),
        fs::read(roaming.join("known_hosts")).unwrap()
    );
}

#[test]
fn portable_connections_take_precedence_over_roaming_connections() {
    let portable = directory("portable-source");
    let roaming = directory("secondary-roaming-source");
    legacy_store(&portable, "Portable server");
    legacy_store(&roaming, "Roaming server");

    let migrated = Store::load_portable(&portable, Some(&roaming)).unwrap();

    assert_eq!(migrated.connections()[0].name, "Portable server");
}

#[test]
fn existing_latch_configuration_takes_precedence_without_reading_legacy_file() {
    let portable = directory("existing-latch");
    let mut store = Store::load(portable.join("latch.json")).unwrap();
    store.set_sort_mode(SortMode::MostUsed).unwrap();
    fs::write(portable.join("connections.json"), b"malformed legacy file").unwrap();

    let restored = Store::load_portable(&portable, None).unwrap();

    assert_eq!(restored.sort_mode(), SortMode::MostUsed);
}

#[test]
fn malformed_latch_configuration_is_never_replaced_by_legacy_configuration() {
    let portable = directory("malformed-latch");
    legacy_store(&portable, "Legacy server");
    let path = portable.join("latch.json");
    fs::write(&path, b"malformed portable file").unwrap();

    let error = Store::load_portable(&portable, None).err().unwrap();

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(fs::read(path).unwrap(), b"malformed portable file");
}

#[test]
fn malformed_legacy_configuration_is_preserved_without_creating_latch_file() {
    let portable = directory("malformed-legacy");
    fs::create_dir_all(&portable).unwrap();
    fs::write(portable.join("connections.json"), b"malformed legacy file").unwrap();

    let error = Store::load_portable(&portable, None).err().unwrap();

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(!portable.join("latch.json").exists());
    assert_eq!(
        fs::read(portable.join("connections.json")).unwrap(),
        b"malformed legacy file"
    );
}

#[test]
fn migration_preserves_existing_portable_host_trust() {
    let roaming = directory("host-trust-source");
    let portable = directory("host-trust-destination");
    legacy_store(&roaming, "Roaming server");
    fs::write(roaming.join("known_hosts"), b"old synthetic host").unwrap();
    fs::create_dir_all(&portable).unwrap();
    fs::write(portable.join("known_hosts"), b"current synthetic host").unwrap();

    Store::load_portable(&portable, Some(&roaming)).unwrap();

    assert_eq!(
        fs::read(portable.join("known_hosts")).unwrap(),
        b"current synthetic host"
    );
}

#[test]
fn new_portable_settings_are_saved_under_latch_json() {
    let portable = directory("fresh-portable");
    let mut store = Store::load_portable(&portable, None).unwrap();

    store.set_sort_mode(SortMode::MostUsed).unwrap();

    assert!(portable.join("latch.json").exists());
    assert!(!portable.join("connections.json").exists());
}
