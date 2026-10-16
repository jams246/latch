use std::{fs, io, path::PathBuf};

use uuid::Uuid;

use super::{AuthMethod, Connection, SortMode, Store};

pub(super) fn store(tag: &str) -> Store {
    Store::load(path(tag)).unwrap()
}

pub(super) fn empty_snapshot() -> super::ImportSnapshot {
    Store::parse_import(br#"{"format_version":1,"configuration":{"connections":[],"sort_mode":"name","terminal_theme":"classic","window_size":null},"local_directories":{}}"#).unwrap()
}

pub(super) fn path(tag: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/encrypted-storage/storage")
        .join(format!("{tag}-{}", Uuid::new_v4()))
        .join("latch.db")
}

fn sample() -> Connection {
    Connection::new(
        "Synthetic server".into(),
        "127.0.0.1".into(),
        "synthetic-user".into(),
        AuthMethod::KeyFile {
            path: "existing-synthetic-key".into(),
        },
    )
}

#[test]
fn connections_counts_and_all_preferences_survive_restart() {
    let path = path("restart");
    let mut store = Store::load(&path).unwrap();
    let original = sample();
    store.upsert(original.clone()).unwrap();
    store.record_successful_shell(&original.id).unwrap();
    for mode in [
        SortMode::Name,
        SortMode::MostUsed,
        SortMode::FrequentAndName,
    ] {
        store.set_sort_mode(mode).unwrap();
        let restored = Store::load(&path).unwrap();
        let connection = &restored.connections()[0];
        assert_eq!(restored.sort_mode(), mode);
        assert_eq!(
            (
                &connection.id,
                connection.port,
                connection.successful_shells
            ),
            (&original.id, 22, 1)
        );
    }
}

#[test]
fn malformed_configuration_is_preserved_without_disclosing_contents() {
    let path = path("malformed");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = br#"{"password":"synthetic-secret", invalid}"#;
    fs::write(&path, original).unwrap();
    let error = Store::load(&path).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(!error.to_string().contains("synthetic-secret"));
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn plaintext_settings_are_rejected_as_an_encrypted_database() {
    let path = path("unknown-field");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        br#"{"connections":[],"password":"synthetic-secret"}"#,
    )
    .unwrap();
    assert_eq!(
        Store::load(&path).err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn serialized_metadata_contains_only_allowed_fields() {
    let mut store = store("metadata-only");
    store.upsert(sample()).unwrap();
    let bytes = store.export_json().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut fields: Vec<_> = value["configuration"]["connections"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "auth",
            "host",
            "id",
            "name",
            "port",
            "successful_shells",
            "username"
        ]
    );
    assert_eq!(
        value["configuration"]["connections"][0]["auth"],
        serde_json::json!({"method":"key_file","path":"existing-synthetic-key"})
    );
}

#[test]
fn failed_atomic_replacement_preserves_memory_and_cleans_temporary_file() {
    let mut store = store("failed-write");
    fs::create_dir_all(&store.path).unwrap();
    assert!(store.upsert(sample()).is_err());
    assert!(store.connections().is_empty());
    let siblings: Vec<_> = fs::read_dir(store.path.parent().unwrap())
        .unwrap()
        .collect();
    assert_eq!(siblings.len(), 1);
}

#[cfg(windows)]
#[test]
fn locked_configuration_preserves_prior_file_and_preference() {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};

    let mut store = store("locked-file");
    store.upsert(sample()).unwrap();
    let original = fs::read(&store.path).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&store.path)
        .unwrap();
    assert!(store.set_sort_mode(SortMode::MostUsed).is_err());
    assert_eq!(store.sort_mode(), SortMode::Name);
    drop(lock);
    assert_eq!(fs::read(&store.path).unwrap(), original);
}

#[test]
fn duplicate_ids_and_invalid_connection_fields_are_rejected() {
    let mut store = store("validation");
    let original = sample();
    store.upsert(original.clone()).unwrap();
    let mut invalid = original.clone();
    invalid.port = 0;
    assert!(store.upsert(invalid).is_err());
    let mut value: serde_json::Value =
        serde_json::from_slice(&store.export_json().unwrap()).unwrap();
    value["configuration"]["connections"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::to_value(&original).unwrap());
    assert_eq!(
        Store::parse_import(&serde_json::to_vec(&value).unwrap())
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn imported_connections_require_port_and_usage_count() {
    for field in ["port", "successful_shells"] {
        let mut value: serde_json::Value =
            serde_json::from_slice(&empty_snapshot().export_json().unwrap()).unwrap();
        value["configuration"]["connections"] = serde_json::json!([sample()]);
        value["configuration"]["connections"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            Store::parse_import(&serde_json::to_vec(&value).unwrap())
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::InvalidData,
            "missing {field}"
        );
    }
}

#[test]
fn imported_window_size_requires_maximized_state() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&empty_snapshot().export_json().unwrap()).unwrap();
    value["configuration"]["window_size"] = serde_json::json!({"width": 1120., "height": 720.});
    assert_eq!(
        Store::parse_import(&serde_json::to_vec(&value).unwrap())
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::InvalidData
    );
}
