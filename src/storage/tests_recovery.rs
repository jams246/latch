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
fn incomplete_encrypted_directory_never_reimports_retained_json() {
    let directory = directory("incomplete-profile");
    fs::create_dir_all(directory.join("latch-data")).unwrap();
    fs::write(directory.join("latch.json"), b"{}").unwrap();
    assert_eq!(
        Store::load_portable(&directory, None).err().unwrap().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(!directory.join("latch-data/key.dpapi").exists());
}

#[test]
fn archived_profile_without_active_directory_never_reimports_retained_json() {
    let directory = directory("archived-without-active");
    fs::create_dir_all(directory.join("latch-data.archive-original")).unwrap();
    fs::write(directory.join("latch.json"), b"{}").unwrap();
    assert!(Store::load_portable(&directory, None).is_err());
    assert!(!directory.join("latch-data").exists());
}

#[test]
fn pending_recovery_without_active_profile_blocks_stale_json_import() {
    let directory = directory("interrupted-recovery");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("latch-recovery.pending"),
        b"LATCH-RECOVERY-V1\n",
    )
    .unwrap();
    fs::write(directory.join("latch.json"), b"{}").unwrap();
    assert!(Store::load_portable(&directory, None).is_err());
    assert!(!directory.join("latch-data").exists());
    let recovered =
        Store::recover_portable(&directory, Store::parse_import(b"{}").unwrap()).unwrap();
    assert!(recovered.connections().is_empty());
    assert!(!directory.join("latch-recovery.pending").exists());
}

#[test]
fn recovery_archives_damaged_pair_and_preserves_host_trust_and_legacy_files() {
    let directory = directory("recovery-archive");
    let original = Store::load_portable(&directory, None).unwrap();
    drop(original);
    let key = fs::read(directory.join("latch-data/key.dpapi")).unwrap();
    fs::write(
        directory.join("latch-data/latch.db"),
        b"damaged encrypted database",
    )
    .unwrap();
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
    let archive = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("latch-data.archive-")
        })
        .unwrap();
    assert_eq!(
        fs::read(archive.join("latch.db")).unwrap(),
        b"damaged encrypted database"
    );
    assert_eq!(fs::read(archive.join("key.dpapi")).unwrap(), key);
    assert_ne!(
        fs::read(directory.join("latch-data/key.dpapi")).unwrap(),
        key
    );
}

#[test]
fn failed_recovery_guard_preserves_original_active_profile() {
    let directory = directory("failed-recovery-guard");
    let original = Store::load_portable(&directory, None).unwrap();
    drop(original);
    let bytes = fs::read(directory.join("latch-data/latch.db")).unwrap();
    fs::create_dir(directory.join("latch-recovery.pending")).unwrap();
    assert!(Store::recover_portable(&directory, Store::parse_import(b"{}").unwrap()).is_err());
    assert_eq!(
        fs::read(directory.join("latch-data/latch.db")).unwrap(),
        bytes
    );
}

#[test]
fn recovery_marker_hard_link_cannot_truncate_active_database() {
    let directory = directory("hard-linked-marker");
    let original = Store::load_portable(&directory, None).unwrap();
    drop(original);
    let database = directory.join("latch-data/latch.db");
    let bytes = fs::read(&database).unwrap();
    fs::hard_link(&database, directory.join("latch-recovery.pending")).unwrap();
    assert!(Store::recover_portable(&directory, Store::parse_import(b"{}").unwrap()).is_err());
    assert_eq!(fs::read(database).unwrap(), bytes);
}

#[test]
fn recovery_crash_boundaries_preserve_sources_and_fail_closed_until_publication_finishes() {
    for checkpoint in 0..=5 {
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
            fs::rename(
                directory.join("latch-data"),
                directory.join("latch-data.archive-original"),
            )
            .unwrap();
        }
        if checkpoint >= 4 {
            fs::rename(stage, directory.join("latch-data")).unwrap();
        }
        if checkpoint >= 5 {
            fs::remove_file(marker).unwrap();
        }
        let reopened = Store::load_portable(&directory, None);
        assert_eq!(reopened.is_ok(), checkpoint == 1 || checkpoint == 5);
        assert_eq!(fs::read(directory.join("latch.json")).unwrap(), b"{}");
        if checkpoint >= 3 {
            assert!(
                directory
                    .join("latch-data.archive-original/key.dpapi")
                    .exists()
            );
        }
    }
}
