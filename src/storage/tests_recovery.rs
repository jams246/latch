use std::{fs, io};

use super::{Store, tests_portable::directory};

#[test]
fn process_lock_prevents_second_profile_owner_until_drop() {
    let directory = directory("profile-lock");
    let original = Store::load_portable(&directory, None).unwrap();
    assert_eq!(
        Store::load_portable(&directory, None).err().unwrap().kind(),
        io::ErrorKind::WouldBlock
    );
    drop(original);
    assert!(Store::load_portable(&directory, None).is_ok());
}

#[test]
fn incomplete_root_pair_never_reimports_retained_json() {
    let directory = directory("incomplete-profile");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("latch.db"), b"incomplete database").unwrap();
    fs::write(directory.join("latch.json"), b"{}").unwrap();
    assert_eq!(
        Store::load_portable(&directory, None).err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(!directory.join("key.dpapi").exists());
}

#[test]
fn archived_profile_without_root_pair_never_reimports_retained_json() {
    let directory = directory("archived-without-active");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("latch.db.archive-original"),
        b"preserved archive",
    )
    .unwrap();
    fs::write(directory.join("latch.json"), b"{}").unwrap();
    assert!(Store::load_portable(&directory, None).is_err());
    assert!(!directory.join("latch.db").exists());
}

#[test]
fn pending_recovery_without_root_pair_blocks_stale_json_import() {
    let directory = directory("interrupted-recovery");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("latch-recovery.pending"),
        b"LATCH-RECOVERY-V1\n",
    )
    .unwrap();
    fs::write(directory.join("latch.json"), b"{}").unwrap();
    assert!(Store::load_portable(&directory, None).is_err());
    assert!(!directory.join("latch.db").exists());
    let recovered =
        Store::recover_portable(&directory, Store::parse_import(b"{}").unwrap()).unwrap();
    assert!(recovered.connections().is_empty());
    assert!(!directory.join("latch-recovery.pending").exists());
}

#[test]
fn recovery_archives_damaged_root_pair_and_sidecars_without_new_directories() {
    let directory = directory("recovery-archive");
    let original = Store::load_portable(&directory, None).unwrap();
    drop(original);
    let key = fs::read(directory.join("key.dpapi")).unwrap();
    fs::write(directory.join("latch.db"), b"damaged encrypted database").unwrap();
    for name in ["latch.db-journal", "latch.db-wal", "latch.db-shm"] {
        fs::write(directory.join(name), name).unwrap();
    }
    fs::write(directory.join("known_hosts"), b"synthetic trusted host").unwrap();
    fs::write(
        directory.join("latch.json"),
        b"original retained legacy data",
    )
    .unwrap();
    let recovered =
        Store::recover_portable(&directory, Store::parse_import(b"{}").unwrap()).unwrap();
    assert!(recovered.connections().is_empty());
    assert_eq!(
        fs::read(directory.join("known_hosts")).unwrap(),
        b"synthetic trusted host"
    );
    assert_eq!(
        fs::read(directory.join("latch.json")).unwrap(),
        b"original retained legacy data"
    );
    for (name, bytes) in [
        ("latch.db", b"damaged encrypted database".as_slice()),
        ("key.dpapi", key.as_slice()),
        ("latch.db-journal", b"latch.db-journal"),
        ("latch.db-wal", b"latch.db-wal"),
        ("latch.db-shm", b"latch.db-shm"),
    ] {
        let archive = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(&format!("{name}.archive-"))
            })
            .unwrap();
        assert_eq!(fs::read(archive).unwrap(), bytes);
    }
    assert_ne!(fs::read(directory.join("key.dpapi")).unwrap(), key);
    assert!(
        !fs::read_dir(&directory)
            .unwrap()
            .any(|entry| entry.unwrap().path().is_dir())
    );
}

#[test]
fn failed_recovery_guard_preserves_original_root_profile() {
    let directory = directory("failed-recovery-guard");
    let original = Store::load_portable(&directory, None).unwrap();
    drop(original);
    let bytes = fs::read(directory.join("latch.db")).unwrap();
    fs::create_dir(directory.join("latch-recovery.pending")).unwrap();
    assert!(Store::recover_portable(&directory, Store::parse_import(b"{}").unwrap()).is_err());
    assert_eq!(fs::read(directory.join("latch.db")).unwrap(), bytes);
}

#[test]
fn recovery_marker_hard_link_cannot_truncate_root_database() {
    let directory = directory("hard-linked-marker");
    let original = Store::load_portable(&directory, None).unwrap();
    drop(original);
    let database = directory.join("latch.db");
    let bytes = fs::read(&database).unwrap();
    fs::hard_link(&database, directory.join("latch-recovery.pending")).unwrap();
    assert!(Store::recover_portable(&directory, Store::parse_import(b"{}").unwrap()).is_err());
    assert_eq!(fs::read(database).unwrap(), bytes);
}

#[test]
fn recovery_preserves_unexpected_old_storage_file_and_publishes_root_pair() {
    let directory = directory("unexpected-old-storage-file");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("latch-data"),
        b"original unexpected storage file",
    )
    .unwrap();
    for name in [
        "latch-data.stage-unexpected",
        "latch-data.archive-unexpected",
    ] {
        fs::write(directory.join(name), b"preserved unexpected artifact").unwrap();
    }
    let recovered =
        Store::recover_portable(&directory, Store::parse_import(b"{}").unwrap()).unwrap();
    assert!(recovered.connections().is_empty());
    assert_eq!(
        fs::read(directory.join("latch-data")).unwrap(),
        b"original unexpected storage file"
    );
    assert!(directory.join("latch.db").is_file());
    assert!(directory.join("key.dpapi").is_file());
    for name in [
        "latch-data.stage-unexpected",
        "latch-data.archive-unexpected",
    ] {
        assert_eq!(
            fs::read(directory.join(name)).unwrap(),
            b"preserved unexpected artifact"
        );
    }
}

#[test]
fn recovery_crash_boundaries_preserve_sources_and_fail_closed_until_both_files_publish() {
    for checkpoint in 0..=6 {
        let directory = directory(&format!("recovery-checkpoint-{checkpoint}"));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("latch.json"), b"{}").unwrap();
        if checkpoint == 0 {
            fs::create_dir(directory.join("latch-data.stage-incomplete")).unwrap();
            assert!(Store::load_portable(&directory, None).is_err());
            continue;
        }
        let original = Store::load_portable(&directory, None).unwrap();
        drop(original);
        let stage = super::recovery::stage(&directory, &super::Configuration::default()).unwrap();
        let marker = directory.join("latch-recovery.pending");
        if checkpoint >= 2 {
            fs::write(&marker, b"LATCH-RECOVERY-V1\n").unwrap();
        }
        if checkpoint >= 3 {
            super::pair::archive(&directory, &directory).unwrap();
        }
        if checkpoint >= 4 {
            super::files::copy_new(&stage.join("latch.db"), &directory.join("latch.db")).unwrap();
        }
        if checkpoint >= 5 {
            super::files::copy_new(&stage.join("key.dpapi"), &directory.join("key.dpapi")).unwrap();
        }
        if checkpoint >= 6 {
            super::pair::cleanup(&stage).unwrap();
            fs::remove_file(marker).unwrap();
        }
        let reopened = Store::load_portable(&directory, None);
        assert_eq!(reopened.is_ok(), checkpoint == 1 || checkpoint == 6);
        assert_eq!(fs::read(directory.join("latch.json")).unwrap(), b"{}");
        if checkpoint >= 3 {
            assert!(super::pair::leftovers(&directory).unwrap());
        }
    }
}
