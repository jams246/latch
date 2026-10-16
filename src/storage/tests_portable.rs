use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::{AuthMethod, Configuration, Connection, SortMode, Store, tests_persistence::path};

pub(super) fn directory(tag: &str) -> PathBuf {
    path(tag).parent().unwrap().to_path_buf()
}

fn legacy_store(directory: &Path, filename: &str, name: &str) -> PathBuf {
    fs::create_dir_all(directory).unwrap();
    let mut connection = Connection::new(
        name.into(),
        "127.0.0.1".into(),
        "synthetic-user".into(),
        AuthMethod::Password,
    );
    connection.successful_shells = 1;
    let configuration = Configuration {
        connections: vec![connection],
        sort_mode: SortMode::FrequentAndName,
        ..Default::default()
    };
    let path = directory.join(filename);
    fs::write(&path, serde_json::to_vec_pretty(&configuration).unwrap()).unwrap();
    path
}

#[test]
fn roaming_configuration_and_host_trust_migrate_without_changing_originals() {
    let roaming = directory("roaming-source");
    let portable = directory("portable-destination");
    let legacy = legacy_store(&roaming, "connections.json", "Roaming server");
    fs::write(roaming.join("known_hosts"), b"synthetic known host").unwrap();
    let original = fs::read(&legacy).unwrap();
    let migrated = Store::load_portable(&portable, Some(&roaming)).unwrap();
    assert_eq!(migrated.path, portable.join("latch-data/latch.db"));
    assert_eq!(migrated.connections()[0].name, "Roaming server");
    assert_eq!(migrated.connections()[0].successful_shells, 1);
    assert_eq!(migrated.sort_mode(), SortMode::FrequentAndName);
    assert_eq!(fs::read(&legacy).unwrap(), original);
    assert!(!migrated.warnings().is_empty());
    assert_eq!(
        fs::read(portable.join("known_hosts")).unwrap(),
        fs::read(roaming.join("known_hosts")).unwrap()
    );
    drop(migrated);
    assert!(
        !Store::load_portable(&portable, Some(&roaming))
            .unwrap()
            .warnings()
            .is_empty()
    );
}

#[test]
fn portable_connections_take_precedence_over_roaming_connections() {
    let portable = directory("portable-source");
    let roaming = directory("secondary-roaming-source");
    legacy_store(&portable, "connections.json", "Portable server");
    legacy_store(&roaming, "connections.json", "Roaming server");
    let migrated = Store::load_portable(&portable, Some(&roaming)).unwrap();
    assert_eq!(migrated.connections()[0].name, "Portable server");
}

#[test]
fn latch_json_precedes_older_connections_and_existing_database_precedes_both() {
    let portable = directory("existing-latch");
    let legacy = legacy_store(&portable, "latch.json", "Current server");
    fs::write(portable.join("connections.json"), b"malformed legacy file").unwrap();
    let migrated = Store::load_portable(&portable, None).unwrap();
    assert_eq!(migrated.connections()[0].name, "Current server");
    drop(migrated);
    fs::write(legacy, b"malformed retained JSON").unwrap();
    let restored = Store::load_portable(&portable, None).unwrap();
    assert_eq!(restored.connections()[0].name, "Current server");
}

#[test]
fn malformed_latch_configuration_is_never_replaced_by_legacy_configuration() {
    let portable = directory("malformed-latch");
    legacy_store(&portable, "connections.json", "Legacy server");
    let path = portable.join("latch.json");
    fs::write(&path, b"malformed portable file").unwrap();
    let error = Store::load_portable(&portable, None).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(fs::read(path).unwrap(), b"malformed portable file");
    assert!(!portable.join("latch-data").exists());
}

#[test]
fn malformed_legacy_configuration_is_preserved_without_creating_database() {
    let portable = directory("malformed-legacy");
    fs::create_dir_all(&portable).unwrap();
    fs::write(portable.join("connections.json"), b"malformed legacy file").unwrap();
    assert_eq!(
        Store::load_portable(&portable, None).err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(!portable.join("latch-data").exists());
    assert_eq!(
        fs::read(portable.join("connections.json")).unwrap(),
        b"malformed legacy file"
    );
}

#[test]
fn migration_preserves_existing_portable_host_trust() {
    let roaming = directory("host-trust-source");
    let portable = directory("host-trust-destination");
    legacy_store(&roaming, "connections.json", "Roaming server");
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
fn fresh_portable_profile_creates_only_encrypted_pair_and_lock() {
    let portable = directory("fresh-portable");
    let mut store = Store::load_portable(&portable, None).unwrap();
    store.set_sort_mode(SortMode::MostUsed).unwrap();
    assert!(portable.join("latch-data/latch.db").exists());
    assert!(portable.join("latch-data/key.dpapi").exists());
    assert!(!portable.join("latch.json").exists());
    assert!(!portable.join("connections.json").exists());
}

#[test]
fn local_directories_migrate_by_stable_connection_id() {
    let portable = directory("legacy-directories");
    let legacy = legacy_store(&portable, "latch.json", "Directory server");
    let configuration: Configuration = serde_json::from_slice(&fs::read(legacy).unwrap()).unwrap();
    let id = &configuration.connections[0].id;
    let encoded: String = id.bytes().map(|byte| format!("{byte:02x}")).collect();
    let preferences = portable.join("sftp-local-directories");
    fs::create_dir_all(&preferences).unwrap();
    let remembered = portable.join("synthetic-local");
    fs::write(
        preferences.join(format!("{encoded}.json")),
        serde_json::to_vec(&remembered).unwrap(),
    )
    .unwrap();
    let migrated = Store::load_portable(&portable, None).unwrap();
    assert_eq!(migrated.local_directory(id), Some(remembered.as_path()));
}

#[test]
fn malformed_optional_local_directory_is_preserved_and_reported_without_blocking_migration() {
    let portable = directory("malformed-optional-directory");
    let legacy = legacy_store(&portable, "latch.json", "Directory server");
    let configuration: Configuration = serde_json::from_slice(&fs::read(legacy).unwrap()).unwrap();
    let id = &configuration.connections[0].id;
    let encoded: String = id.bytes().map(|byte| format!("{byte:02x}")).collect();
    let preferences = portable.join("sftp-local-directories");
    fs::create_dir_all(&preferences).unwrap();
    let preference = preferences.join(format!("{encoded}.json"));
    fs::write(&preference, b"invalid optional preference").unwrap();
    let migrated = Store::load_portable(&portable, None).unwrap();
    assert!(migrated.local_directory(id).is_none());
    assert!(
        migrated
            .warnings()
            .iter()
            .any(|warning| warning.contains("Cannot import local directory preference"))
    );
    assert_eq!(
        fs::read(preference).unwrap(),
        b"invalid optional preference"
    );
}
