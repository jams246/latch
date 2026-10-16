use super::*;

pub(super) fn fixture(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/sftp-dual-pane/local")
        .join(format!("{name}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn listing_omits_directory_sizes_and_preserves_file_metadata() {
    let directory = fixture("listing");
    fs::create_dir(directory.join("Folder")).unwrap();
    fs::write(directory.join("résumé.txt"), b"contents").unwrap();
    let listing = list(&directory).unwrap();
    let folder = listing
        .entries
        .iter()
        .find(|entry| entry.metadata.name == "Folder")
        .unwrap();
    let file = listing
        .entries
        .iter()
        .find(|entry| entry.metadata.name == "résumé.txt")
        .unwrap();
    assert!(folder.metadata.is_dir);
    assert_eq!(folder.metadata.size, None);
    assert_eq!(file.metadata.size, Some(8));
    assert!(file.metadata.modified.is_some());
    assert_eq!(file.path, directory.join("résumé.txt"));
}

#[test]
fn initial_listing_skips_unusable_candidates_until_a_directory_can_be_read() {
    let directory = fixture("initial-fallback");
    let file = directory.join("file.txt");
    fs::write(&file, b"sample").unwrap();
    let listing = list_first_available(vec![
        directory.join("missing"),
        file.clone(),
        file,
        directory.clone(),
    ])
    .unwrap();
    assert_eq!(listing.path, directory);
}

#[cfg(windows)]
fn shortcut(path: &Path, target: &Path) {
    use windows::{
        Win32::{
            System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile},
            UI::Shell::{IShellLinkW, ShellLink},
        },
        core::{Interface, PCWSTR},
    };
    let _apartment = native::Apartment::new().unwrap();
    let target = native::wide(target).unwrap();
    let path = native::wide(path).unwrap();
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
        link.SetPath(PCWSTR(target.as_ptr())).unwrap();
        let file: IPersistFile = link.cast().unwrap();
        file.Save(PCWSTR(path.as_ptr()), true).unwrap();
    }
}

#[cfg(windows)]
#[test]
fn windows_shortcuts_follow_directory_targets_and_reveal_file_targets() {
    let directory = fixture("shortcuts");
    let target = directory.join("target");
    fs::create_dir(&target).unwrap();
    let file = target.join("document.txt");
    fs::write(&file, b"sample").unwrap();
    let folder_link = directory.join("Folder.lnk");
    let file_link = directory.join("File.LNK");
    shortcut(&folder_link, &target);
    shortcut(&file_link, &file);
    assert_eq!(follow_link(&folder_link).unwrap(), target);
    assert_eq!(follow_link(&file_link).unwrap(), target);
    let listing = list(&directory).unwrap();
    assert!(
        listing
            .entries
            .iter()
            .filter(|entry| is_shortcut(&entry.path))
            .all(|entry| entry.metadata.is_link)
    );
}

#[cfg(windows)]
#[test]
fn windows_listing_supplies_native_file_icons_and_available_drives() {
    let directory = fixture("icons");
    fs::write(directory.join("document.txt"), b"sample").unwrap();
    let listing = list(&directory).unwrap();
    let icon = listing.entries[0].icon.as_ref().unwrap();
    assert_eq!(icon.size(0), gpui::size(16.into(), 16.into()));
    assert!(
        icon.as_bytes(0)
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[3] > 0)
    );
    assert!(!listing.drives.is_empty());
}

#[cfg(windows)]
#[test]
fn native_icons_survive_concurrent_directory_lists() {
    let directory = fixture("concurrent-icons");
    fs::write(directory.join("document.txt"), b"sample").unwrap();
    let barrier = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let directory = &directory;
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                for _ in 0..4 {
                    assert!(list(directory).unwrap().entries[0].icon.is_some());
                }
            });
        }
    });
}

#[cfg(windows)]
#[test]
fn broken_windows_shortcuts_report_errors_without_executing_targets() {
    let directory = fixture("broken-shortcut");
    let path = directory.join("broken.lnk");
    fs::write(&path, b"invalid Windows shortcut").unwrap();
    assert!(follow_link(&path).is_err());
    assert!(list(&directory).unwrap().entries[0].metadata.is_link);
}

#[cfg(windows)]
#[test]
fn local_symlinks_follow_directories_and_report_broken_targets() {
    let directory = fixture("symlink");
    let target = directory.join("target");
    fs::create_dir(&target).unwrap();
    let path = directory.join("link");
    if let Err(error) = std::os::windows::fs::symlink_dir(&target, &path) {
        if error.raw_os_error() == Some(1314) || error.kind() == io::ErrorKind::PermissionDenied {
            eprintln!(
                "Skipping local symlink checks: Windows symlink creation requires Developer Mode or the create symbolic links privilege: {error}"
            );
            return;
        }
        panic!("Cannot create local symlink fixture: {error}");
    }
    assert_eq!(follow_link(&path).unwrap(), target);
    let entry = list(&directory)
        .unwrap()
        .entries
        .into_iter()
        .find(|entry| entry.path == path)
        .unwrap();
    assert!(entry.metadata.is_link && entry.metadata.is_dir);
    assert_eq!(entry.metadata.size, None);
    fs::remove_dir(&target).unwrap();
    assert!(follow_link(&path).is_err());
}

#[cfg(windows)]
#[test]
fn extended_windows_paths_display_normal_drive_and_unc_breadcrumbs() {
    assert_eq!(
        native::display_path(Path::new(r"\\?\C:\Users")),
        PathBuf::from(r"C:\Users")
    );
    assert_eq!(
        native::display_path(Path::new(r"\\?\UNC\server\share")),
        PathBuf::from(r"\\server\share")
    );
}
