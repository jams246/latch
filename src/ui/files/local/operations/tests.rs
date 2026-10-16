use super::*;

fn fixture(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/sftp-actions/local")
        .join(format!("{name}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn invalid_names_reject_traversal_windows_devices_and_ambiguous_suffixes() {
    for name in [
        "",
        ".",
        "..",
        "../outside",
        "a/b",
        "a\\b",
        "C:outside",
        "file\0name",
        "file?",
        "file.",
        "file ",
        "NUL",
        "con.txt",
        "AUX .txt",
        "COM1.log",
        "LPT9",
        "LPT¹",
        "CONIN$",
        "CONOUT$",
    ] {
        assert_eq!(
            validate_name(name).unwrap_err().kind(),
            io::ErrorKind::InvalidInput,
            "{name:?}"
        );
    }
}

#[test]
fn names_preserve_unicode_and_nonreserved_device_prefixes() {
    for name in [
        "résumé.txt",
        "新しいフォルダー",
        ".gitignore",
        "COM10.txt",
        "console",
    ] {
        assert!(validate_name(name).is_ok(), "{name}");
    }
}

#[test]
fn create_folder_never_replaces_existing_files_or_folders() {
    let directory = fixture("create");
    create_directory(&directory, "新しいフォルダー").unwrap();
    assert!(directory.join("新しいフォルダー").is_dir());
    fs::write(directory.join("existing"), b"keep").unwrap();
    assert!(create_directory(&directory, "existing").is_err());
    assert!(create_directory(&directory, "新しいフォルダー").is_err());
    assert_eq!(fs::read(directory.join("existing")).unwrap(), b"keep");
}

#[test]
fn delete_removes_selected_files_and_nested_folders() {
    let directory = fixture("delete");
    let folder = directory.join("folder");
    fs::create_dir_all(folder.join("nested")).unwrap();
    fs::write(folder.join("nested/child.txt"), b"remove").unwrap();
    let file = directory.join("file.txt");
    fs::write(&file, b"remove").unwrap();
    fs::write(directory.join("keep.txt"), b"keep").unwrap();
    delete(&[file.clone(), folder.clone()]).unwrap();
    assert!(!file.exists() && !folder.exists());
    assert_eq!(fs::read(directory.join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn delete_validates_every_target_before_removing_anything() {
    let directory = fixture("invalid-delete");
    let file = directory.join("keep.txt");
    fs::write(&file, b"keep").unwrap();
    let result = delete(&[file.clone(), directory.join("..")]);
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
    assert_eq!(fs::read(file).unwrap(), b"keep");
}

#[test]
fn target_validation_rejects_roots_and_dot_paths() {
    let root = Path::new("C:\\");
    for path in [root, Path::new("."), Path::new(".."), Path::new("")] {
        assert_eq!(
            target_path(path).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    let directory = fixture("dot-path");
    assert!(target_path(&directory.join(".")).is_err());
    for root in [r"\\server\share", r"\\?\C:\", r"\\?\UNC\server\share\"] {
        assert!(target_path(Path::new(root)).is_err());
    }
}

#[test]
fn rename_preserves_contents_for_files_and_folders() {
    let directory = fixture("rename");
    let source = directory.join("source.txt");
    fs::write(&source, b"contents").unwrap();
    rename(&source, "résumé.txt").unwrap();
    assert!(!source.exists());
    assert_eq!(fs::read(directory.join("résumé.txt")).unwrap(), b"contents");
    create_directory(&directory, "source").unwrap();
    fs::write(directory.join("source/child.txt"), b"nested").unwrap();
    rename(&directory.join("source"), "renamed").unwrap();
    assert_eq!(
        fs::read(directory.join("renamed/child.txt")).unwrap(),
        b"nested"
    );
}

#[test]
fn rename_never_replaces_existing_destination_contents() {
    let directory = fixture("rename-existing");
    let source = directory.join("source.txt");
    fs::write(&source, b"source").unwrap();
    fs::write(directory.join("target.txt"), b"target").unwrap();
    assert_eq!(
        rename(&source, "target.txt").unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&source).unwrap(), b"source");
    assert_eq!(fs::read(directory.join("target.txt")).unwrap(), b"target");
}

#[test]
fn rename_allows_changing_only_letter_case() {
    let directory = fixture("rename-case");
    let source = directory.join("Report.txt");
    fs::write(&source, b"contents").unwrap();
    rename(&source, "report.txt").unwrap();
    assert_eq!(
        fs::read_dir(directory)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name(),
        "report.txt"
    );
}

#[test]
fn unchanged_rename_requires_the_source_to_exist() {
    let directory = fixture("rename-unchanged");
    let source = directory.join("same.txt");
    assert_eq!(
        rename(&source, "same.txt").unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    fs::write(&source, b"keep").unwrap();
    rename(&source, "same.txt").unwrap();
    assert_eq!(fs::read(source).unwrap(), b"keep");
}

#[test]
fn delete_does_not_follow_directory_links_even_inside_deleted_folders() {
    let directory = fixture("delete-links");
    let target = directory.join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep.txt"), b"keep").unwrap();
    let folder = directory.join("folder");
    fs::create_dir(&folder).unwrap();
    let link = directory.join("link");
    if let Err(error) = std::os::windows::fs::symlink_dir(&target, &link) {
        if error.raw_os_error() == Some(1314) || error.kind() == io::ErrorKind::PermissionDenied {
            eprintln!("Skipping symlink deletion check: Windows symlink creation denied: {error}");
            return;
        }
        panic!("Cannot create symlink fixture: {error}");
    }
    std::os::windows::fs::symlink_dir(&target, folder.join("nested-link")).unwrap();
    delete(&[link.clone(), folder.clone()]).unwrap();
    assert!(fs::symlink_metadata(link).is_err() && !folder.exists());
    assert_eq!(fs::read(target.join("keep.txt")).unwrap(), b"keep");
}
