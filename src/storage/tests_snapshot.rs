use std::{fs, io};

use super::{
    AuthMethod, Connection, SortMode, Store, TerminalTheme, WindowSize, tests_persistence::store,
};

fn sample() -> Connection {
    Connection::new(
        "Synthetic server".into(),
        "127.0.0.1".into(),
        "synthetic".into(),
        AuthMethod::KeyFile {
            path: "existing-synthetic-key".into(),
        },
    )
}

#[test]
fn complete_snapshot_round_trip_preserves_counts_paths_and_all_preferences() {
    let mut original = store("complete-export");
    let connection = sample();
    original.upsert(connection.clone()).unwrap();
    original.configuration.connections[0].successful_shells = u64::MAX;
    original.save().unwrap();
    original.record_successful_shell(&connection.id).unwrap();
    original.set_sort_mode(SortMode::FrequentAndName).unwrap();
    original
        .set_terminal_theme(TerminalTheme::Midnight)
        .unwrap();
    original
        .set_window_size(WindowSize {
            width: 1400.,
            height: 900.,
            maximized: true,
        })
        .unwrap();
    let local = original.path.parent().unwrap().join("synthetic-local");
    original
        .set_local_directory(&connection.id, &local)
        .unwrap();
    let mut imported = store("complete-import");
    imported.upsert(sample()).unwrap();
    imported.save().unwrap();
    let wrapped_key = fs::read(imported.path.with_file_name("key.dpapi")).unwrap();
    imported
        .replace_import(Store::parse_import(&original.export_json().unwrap()).unwrap())
        .unwrap();
    assert!(original.configuration == imported.configuration);
    assert!(Store::load(&imported.path).unwrap().configuration == original.configuration);
    assert_eq!(imported.connections().len(), 1);
    assert_eq!(
        fs::read(imported.path.with_file_name("key.dpapi")).unwrap(),
        wrapped_key
    );
}

#[test]
fn legacy_import_defaults_missing_preferences_and_rejects_unknown_fields() {
    let mut imported = store("legacy-import");
    imported.upsert(sample()).unwrap();
    imported
        .replace_import(Store::parse_import(br#"{"connections":[]}"#).unwrap())
        .unwrap();
    assert!(imported.connections().is_empty());
    assert_eq!(imported.sort_mode(), SortMode::Name);
    assert_eq!(imported.terminal_theme(), TerminalTheme::Classic);
    assert!(imported.configuration.local_directories.is_empty());
    assert!(Store::parse_import(br#"{"password":"synthetic-secret"}"#).is_err());
}

#[test]
fn unknown_version_and_invalid_connections_never_fall_back_to_legacy() {
    let original = store("invalid-import");
    let mut value: serde_json::Value =
        serde_json::from_slice(&original.export_json().unwrap()).unwrap();
    value["format_version"] = 2.into();
    assert!(Store::parse_import(&serde_json::to_vec(&value).unwrap()).is_err());
    value["format_version"] = 1.into();
    value["configuration"]["connections"] = serde_json::json!([sample(), sample()]);
    value["configuration"]["connections"][1]["id"] =
        value["configuration"]["connections"][0]["id"].clone();
    assert!(Store::parse_import(&serde_json::to_vec(&value).unwrap()).is_err());
    value["configuration"]["connections"] = serde_json::json!([]);
    value["configuration"]["password"] = "synthetic-secret".into();
    let error = Store::parse_import(&serde_json::to_vec(&value).unwrap())
        .err()
        .unwrap();
    assert!(!error.to_string().contains("synthetic-secret"));
}

#[test]
fn duplicate_or_orphan_directory_preferences_are_rejected() {
    let bytes = br#"{"format_version":1,"configuration":{"connections":[],"sort_mode":"name","terminal_theme":"classic","window_size":null},"local_directories":{"a":"x","a":"y"}}"#;
    assert!(Store::parse_import(bytes).is_err());
    let bytes = br#"{"format_version":1,"configuration":{"connections":[],"sort_mode":"name","terminal_theme":"classic","window_size":null},"local_directories":{"missing":"x"}}"#;
    assert!(Store::parse_import(bytes).is_err());
}

#[test]
fn local_directory_changes_preserve_names_and_delete_removes_preference() {
    let mut original = store("directory-lifecycle");
    let mut connection = sample();
    original.upsert(connection.clone()).unwrap();
    let local = original.path.parent().unwrap().join("local");
    original
        .set_local_directory(&connection.id, &local)
        .unwrap();
    connection.name = "Renamed".into();
    original.upsert(connection.clone()).unwrap();
    assert_eq!(
        original.local_directory(&connection.id),
        Some(local.as_path())
    );
    assert!(original.set_local_directory("missing", &local).is_err());
    original.delete(&connection.id).unwrap();
    assert!(original.local_directory(&connection.id).is_none());
}

#[test]
fn oversized_import_is_bounded() {
    let bytes = vec![b' '; Store::MAX_SNAPSHOT_BYTES + 1];
    assert_eq!(
        Store::parse_import(&bytes).err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn settings_that_cannot_fit_export_limit_are_rejected_before_commit() {
    let mut original = store("oversized-update");
    let mut connection = sample();
    connection.name = "x".repeat(Store::MAX_SNAPSHOT_BYTES);
    assert!(original.upsert(connection).is_err());
    assert!(original.connections().is_empty());
    assert!(!original.path.exists());
}

#[test]
fn exports_protect_internal_files_and_allow_user_backup_destination() {
    let directory = super::tests_portable::directory("export-targets");
    let original = Store::load_portable(&directory, None).unwrap();
    let bytes = original.export_json().unwrap();
    for name in [
        "latch.db",
        "key.dpapi",
        "latch.db-journal",
        "latch.db-wal",
        "latch.db-shm",
        "latch.db.archive-synthetic",
        "key.dpapi.archive-synthetic",
        "latch.db-journal.archive-synthetic",
        "latch.db-wal.archive-synthetic",
        "latch.db-shm.archive-synthetic",
        "latch-data/latch.db",
        "latch-data/key.dpapi",
        "latch-data.stage-synthetic/latch.db",
        "latch-data.archive-synthetic/key.dpapi",
        "latch.profile.lock",
        "latch-recovery.pending",
        "known_hosts",
    ] {
        let path = directory.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        if !path.exists() {
            fs::write(&path, b"synthetic internal data").unwrap();
        }
        let previous = (name != super::locking::LOCK_FILE).then(|| fs::read(&path).unwrap());
        let metadata = fs::metadata(&path).unwrap();
        let error = Store::write_export(&directory, &path, &bytes).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name}");
        if let Some(previous) = previous {
            assert_eq!(fs::read(path).unwrap(), previous, "{name}");
        } else {
            let after = fs::metadata(path).unwrap();
            assert_eq!(after.len(), metadata.len());
            assert_eq!(after.modified().unwrap(), metadata.modified().unwrap());
        }
    }
    let backup = directory.join("latch-backup.json");
    Store::write_export(&directory, &backup, &bytes).unwrap();
    let restored = Store::read_import_file(&backup)
        .unwrap()
        .into_configuration();
    assert!(original.configuration == restored);
}

#[test]
fn exports_reserve_missing_flat_files_and_preserve_unrelated_backup_paths() {
    let directory = super::tests_portable::directory("missing-export-targets");
    fs::create_dir_all(&directory).unwrap();
    for name in [
        "LATCH.DB",
        "KEY.DPAPI",
        "latch.db-wal",
        "latch.db.archive-new",
    ] {
        let path = directory.join(name);
        let error = Store::write_export(&directory, &path, b"{}").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name}");
        assert!(!path.exists(), "{name}");
    }
    let backup = directory.join("backups/latch.db");
    fs::create_dir_all(backup.parent().unwrap()).unwrap();
    Store::write_export(&directory, &backup, b"{}").unwrap();
    assert_eq!(fs::read(backup).unwrap(), b"{}");
}

#[cfg(windows)]
#[test]
fn exports_reject_flat_file_names_with_windows_ignored_trailing_characters() {
    let directory = super::tests_portable::directory("aliased-export-targets");
    fs::create_dir_all(&directory).unwrap();
    for name in ["latch.db.", "key.dpapi ", "latch.db-wal. "] {
        let error = Store::write_export(&directory, &directory.join(name), b"{}").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name}");
    }
    assert_eq!(fs::read_dir(directory).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn failed_import_preserves_configuration_and_wrapped_key() {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};
    let mut original = store("failed-import");
    original.upsert(sample()).unwrap();
    let previous = original.configuration.clone();
    let bytes = fs::read(&original.path).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&original.path)
        .unwrap();
    assert!(
        original
            .replace_import(Store::parse_import(b"{}").unwrap())
            .is_err()
    );
    assert!(original.configuration == previous);
    drop(lock);
    assert_eq!(fs::read(original.path).unwrap(), bytes);
}
