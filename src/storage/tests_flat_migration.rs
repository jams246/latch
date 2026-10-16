use std::{fs, io};

use super::{AuthMethod, Connection, Store, tests_portable::directory};

fn old_profile(directory: &std::path::Path) -> Store {
    let old = directory.join("latch-data");
    let mut store = Store::load(old.join("latch.db")).unwrap();
    store
        .upsert(Connection::new(
            "Existing encrypted server".into(),
            "synthetic.example".into(),
            "synthetic".into(),
            AuthMethod::Password,
        ))
        .unwrap();
    store
        .record_successful_shell(&store.connections()[0].id.clone())
        .unwrap();
    store
}

#[test]
fn existing_folder_pair_moves_to_profile_root_without_key_rotation_or_metadata_changes() {
    let directory = directory("flat-folder-migration");
    let original = old_profile(&directory);
    let database = fs::read(directory.join("latch-data/latch.db")).unwrap();
    let key = fs::read(directory.join("latch-data/key.dpapi")).unwrap();
    let migrated = Store::load_portable(&directory, None).unwrap();
    assert_eq!(migrated.path, directory.join("latch.db"));
    assert_eq!(migrated.connections(), original.connections());
    assert_eq!(fs::read(directory.join("latch.db")).unwrap(), database);
    assert_eq!(fs::read(directory.join("key.dpapi")).unwrap(), key);
    assert!(!directory.join("latch-data").exists());
    assert!(!directory.join("latch-recovery.pending").exists());
}

#[test]
fn folder_migration_preserves_unknown_files_and_removes_only_pair() {
    let directory = directory("flat-folder-unknown");
    old_profile(&directory);
    let unknown = directory.join("latch-data/retained-user-file");
    fs::write(&unknown, b"preserve unrelated data").unwrap();
    Store::load_portable(&directory, None).unwrap();
    assert_eq!(fs::read(unknown).unwrap(), b"preserve unrelated data");
    assert!(!directory.join("latch-data/latch.db").exists());
    assert!(!directory.join("latch-data/key.dpapi").exists());
}

#[test]
fn folder_migration_cleans_known_sidecars_after_verified_reopen() {
    let directory = directory("flat-folder-sidecars");
    old_profile(&directory);
    fs::write(directory.join("latch-data/latch.db-journal"), b"stale").unwrap();
    Store::load_portable(&directory, None).unwrap();
    assert!(!directory.join("latch-data").exists());
    assert!(!directory.join("latch.db-journal").exists());
}

#[test]
fn existing_root_pair_precedes_even_invalid_legacy_folder() {
    let directory = directory("flat-root-precedence");
    let root = Store::load_portable(&directory, None).unwrap();
    drop(root);
    fs::create_dir(directory.join("latch-data")).unwrap();
    fs::write(
        directory.join("latch-data/latch.db"),
        b"invalid legacy database",
    )
    .unwrap();
    let restored = Store::load_portable(&directory, None).unwrap();
    assert!(restored.connections().is_empty());
    assert_eq!(
        fs::read(directory.join("latch-data/latch.db")).unwrap(),
        b"invalid legacy database"
    );
}

#[test]
fn partial_or_corrupt_root_never_falls_back_to_valid_folder_or_json() {
    for corrupt in [false, true] {
        let directory = directory("flat-root-invalid");
        old_profile(&directory);
        fs::write(
            directory.join("latch.db"),
            b"original invalid root database",
        )
        .unwrap();
        if corrupt {
            fs::copy(
                directory.join("latch-data/key.dpapi"),
                directory.join("key.dpapi"),
            )
            .unwrap();
        }
        fs::write(directory.join("latch.json"), b"{}").unwrap();
        assert!(Store::load_portable(&directory, None).is_err());
        assert_eq!(
            fs::read(directory.join("latch.db")).unwrap(),
            b"original invalid root database"
        );
        assert!(directory.join("latch-data/latch.db").exists());
        assert_eq!(directory.join("key.dpapi").exists(), corrupt);
    }
}

#[test]
fn incomplete_or_corrupt_legacy_folder_never_reimports_json() {
    for corrupt in [false, true] {
        let directory = directory("flat-invalid-folder");
        if corrupt {
            old_profile(&directory);
        } else {
            fs::create_dir_all(directory.join("latch-data")).unwrap();
        }
        fs::write(
            directory.join("latch-data/latch.db"),
            b"original invalid folder database",
        )
        .unwrap();
        fs::write(directory.join("latch.json"), b"{}").unwrap();
        assert!(Store::load_portable(&directory, None).is_err());
        assert!(!directory.join("latch.db").exists());
        assert_eq!(
            fs::read(directory.join("latch-data/latch.db")).unwrap(),
            b"original invalid folder database"
        );
    }
}

#[test]
fn publish_rejects_existing_root_file_without_overwrite_or_source_removal() {
    let directory = directory("flat-no-overwrite");
    let original = old_profile(&directory);
    fs::write(directory.join("key.dpapi"), b"existing root key").unwrap();
    let error = super::recovery::publish(
        &directory.join("latch-data"),
        &directory,
        &original.configuration,
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read(directory.join("key.dpapi")).unwrap(),
        b"existing root key"
    );
    assert!(directory.join("latch-data/latch.db").exists());
    assert!(directory.join("latch-recovery.pending").exists());
}
