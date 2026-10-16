use std::{fs, io, path::PathBuf};

use super::{AuthMethod, Connection, SortMode, Store, tests_persistence::path};

pub(super) fn directory(tag: &str) -> PathBuf {
    path(tag).parent().unwrap().to_path_buf()
}

#[test]
fn fresh_profile_creates_only_flat_encrypted_pair_and_lock() {
    let directory = directory("fresh-profile");
    let mut store = Store::load_profile(&directory).unwrap();
    store.set_sort_mode(SortMode::MostUsed).unwrap();
    assert_eq!(store.path, directory.join("latch.db"));
    assert!(directory.join("key.dpapi").exists());
    assert!(
        !fs::read_dir(&directory)
            .unwrap()
            .any(|entry| entry.unwrap().path().is_dir())
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 3);
}

#[test]
fn current_profile_reopens_same_key_and_metadata() {
    let directory = directory("profile-restart");
    let mut original = Store::load_profile(&directory).unwrap();
    original
        .upsert(Connection::new(
            "Synthetic current server".into(),
            "127.0.0.1".into(),
            "synthetic".into(),
            AuthMethod::Password,
        ))
        .unwrap();
    original.set_sort_mode(SortMode::MostUsed).unwrap();
    let key = fs::read(directory.join("key.dpapi")).unwrap();
    let connections = original.connections().to_vec();
    drop(original);
    let restored = Store::load_profile(&directory).unwrap();
    assert_eq!(restored.connections(), connections);
    assert_eq!(restored.sort_mode(), SortMode::MostUsed);
    assert_eq!(fs::read(directory.join("key.dpapi")).unwrap(), key);
}

#[test]
fn old_json_and_directory_profiles_are_ignored_without_modification() {
    let directory = directory("ignored-old-profile");
    fs::create_dir_all(directory.join("latch-data")).unwrap();
    fs::create_dir(directory.join("latch-data.stage-old")).unwrap();
    fs::create_dir(directory.join("latch-data.archive-old")).unwrap();
    let old_files = [
        "latch.json",
        "connections.json",
        "sftp-local-directory.json",
        "latch-data/latch.db",
        "latch-data/key.dpapi",
    ];
    for name in old_files {
        fs::write(directory.join(name), b"original old data").unwrap();
    }
    let loaded = Store::load_profile(&directory).unwrap();
    assert!(loaded.connections().is_empty());
    for name in old_files {
        assert_eq!(
            fs::read(directory.join(name)).unwrap(),
            b"original old data"
        );
    }
    assert!(directory.join("latch-data.stage-old").is_dir());
    assert!(directory.join("latch-data.archive-old").is_dir());
}

#[test]
fn incomplete_or_corrupt_current_pair_is_preserved_without_regeneration() {
    for corrupt in [false, true] {
        let directory = directory("invalid-current-profile");
        fs::create_dir_all(&directory).unwrap();
        if corrupt {
            let original = Store::load_profile(&directory).unwrap();
            drop(original);
        }
        fs::write(
            directory.join("latch.db"),
            b"original invalid current database",
        )
        .unwrap();
        fs::write(directory.join("latch.json"), b"{}").unwrap();
        assert!(Store::load_profile(&directory).is_err());
        assert_eq!(
            fs::read(directory.join("latch.db")).unwrap(),
            b"original invalid current database"
        );
        assert_eq!(directory.join("key.dpapi").exists(), corrupt);
    }
}

#[test]
fn current_publication_never_overwrites_existing_pair_member() {
    let directory = directory("current-no-overwrite");
    fs::create_dir_all(&directory).unwrap();
    let configuration = super::Configuration::default();
    let stage = super::recovery::stage(&directory, &configuration).unwrap();
    fs::write(directory.join("key.dpapi"), b"existing protected key").unwrap();
    let error = super::recovery::publish(&stage, &directory, &configuration).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read(directory.join("key.dpapi")).unwrap(),
        b"existing protected key"
    );
    assert!(stage.join("latch.db").exists());
    assert!(directory.join("latch-recovery.pending").exists());
}
