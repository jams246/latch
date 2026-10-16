use super::{tests::fixture, *};

#[test]
fn remembered_local_directory_is_listed_without_reading_legacy_preferences() {
    let directory = fixture("remembered-directory");
    let legacy = directory.join("sftp-local-directory.json");
    fs::write(&legacy, b"invalid JSON").unwrap();

    assert_eq!(
        list_initial(Some(directory.clone())).unwrap().path,
        directory
    );
    assert_eq!(fs::read(legacy).unwrap(), b"invalid JSON");
}

#[test]
fn connections_receive_independent_initial_directories() {
    let first = fixture("connection-first");
    let second = fixture("connection-second");

    assert_eq!(list_initial(Some(first.clone())).unwrap().path, first);
    assert_eq!(list_initial(Some(second.clone())).unwrap().path, second);
}

#[test]
fn absent_remembered_directory_uses_an_existing_default() {
    assert!(list_initial(None).unwrap().path.is_dir());
}

#[test]
fn unavailable_remembered_directory_falls_back_to_an_existing_directory() {
    let unavailable = fixture("unavailable").join("missing");
    let selected = list_initial(Some(unavailable.clone())).unwrap().path;

    assert_ne!(selected, unavailable);
    assert!(selected.is_dir());
}

#[test]
fn remembered_file_falls_back_without_modifying_it() {
    let file = fixture("remembered-file").join("file.txt");
    fs::write(&file, b"keep").unwrap();
    let selected = list_initial(Some(file.clone())).unwrap().path;

    assert_ne!(selected, file);
    assert!(selected.is_dir());
    assert_eq!(fs::read(file).unwrap(), b"keep");
}

#[test]
fn deleted_remembered_directory_uses_an_existing_default() {
    let directory = fixture("deleted-directory");
    fs::remove_dir(&directory).unwrap();
    let selected = list_initial(Some(directory.clone())).unwrap().path;

    assert_ne!(selected, directory);
    assert!(selected.is_dir());
}
