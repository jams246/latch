use std::{
    io,
    os::windows::{ffi::OsStrExt, io::FromRawHandle},
    path::Path,
};

use tokio::fs::File;
use windows::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
            FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile, RtlDosPathNameToNtPathName_U_WithStatus,
        },
    },
    Win32::{
        Foundation::{
            HANDLE, NTSTATUS, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, RtlNtStatusToDosError,
            STATUS_FILE_IS_A_DIRECTORY, STATUS_REPARSE_POINT_ENCOUNTERED, UNICODE_STRING,
        },
        Storage::FileSystem::{FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_READ, FILE_SHARE_READ},
        System::{IO::IO_STATUS_BLOCK, WindowsProgramming::RtlFreeUnicodeString},
    },
    core::PCWSTR,
};

use super::{Error, transfer_destination::local_is_link};

pub(super) async fn open(path: &Path) -> Result<File, Error> {
    let path = path.to_owned();
    let file = tokio::task::spawn_blocking(move || open_native(&path))
        .await
        .map_err(|error| Error::Local(io::Error::other(error)))??;
    Ok(File::from_std(file))
}

fn open_native(path: &Path) -> Result<std::fs::File, Error> {
    // Leave room for the NT namespace prefix within UNICODE_STRING's 16-bit byte length.
    let mut input = path.as_os_str().encode_wide();
    if input.clone().take(32_761).count() > 32_760 || input.any(|unit| unit == 0) {
        return Err(Error::InvalidPath);
    }
    let mut wide: Vec<_> = std::path::absolute(path)?
        .as_os_str()
        .encode_wide()
        .collect();
    if wide.len() > 32_760 {
        return Err(Error::InvalidPath);
    }
    wide.push(0);
    let mut name = UNICODE_STRING::default();
    // SAFETY: The input is NUL-terminated and the output remains alive until freed below.
    check(unsafe {
        RtlDosPathNameToNtPathName_U_WithStatus(PCWSTR(wide.as_ptr()), &mut name, None, None)
    })?;
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        ObjectName: &name,
        // The kernel rejects reparses atomically across the complete path, including racing changes.
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        ..Default::default()
    };
    let mut handle = HANDLE::default();
    let mut status_block = IO_STATUS_BLOCK::default();
    // SAFETY: All pointers refer to live initialized storage and the open completes synchronously.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_GENERIC_READ,
            &attributes,
            &mut status_block,
            None,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            FILE_SHARE_READ,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            None,
            0,
        )
    };
    // SAFETY: RtlDosPathNameToNtPathName_U_WithStatus allocated this string on success.
    unsafe { RtlFreeUnicodeString(&mut name) };
    check(status)?;
    // SAFETY: A successful synchronous open returns one owned handle, now owned only by File.
    let file = unsafe { std::fs::File::from_raw_handle(handle.0) };
    let metadata = file.metadata()?;
    if !metadata.is_file() || local_is_link(&metadata) {
        return Err(Error::UnsupportedCopySource);
    }
    Ok(file)
}

fn check(status: NTSTATUS) -> Result<(), Error> {
    if matches!(
        status,
        STATUS_REPARSE_POINT_ENCOUNTERED | STATUS_FILE_IS_A_DIRECTORY
    ) {
        return Err(Error::UnsupportedCopySource);
    }
    if status.0 < 0 {
        // SAFETY: This conversion accepts any NTSTATUS value and allocates no resources.
        return Err(
            io::Error::from_raw_os_error(unsafe { RtlNtStatusToDosError(status) } as i32).into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upload_handle_prevents_source_replacement_and_modification() {
        let path = crate::sftp::tests::directory().join("source.txt");
        std::fs::write(&path, b"Synthetic upload source").unwrap();
        let source = open(&path).await.unwrap();
        assert!(std::fs::remove_file(&path).is_err());
        assert!(std::fs::write(&path, b"Changed contents").is_err());
        drop(source);
        assert_eq!(std::fs::read(&path).unwrap(), b"Synthetic upload source");
    }

    #[tokio::test]
    async fn upload_handle_rejects_symlink_without_reading_its_target() {
        let root = crate::sftp::tests::directory();
        let target = root.join("private.txt");
        let link = root.join("source.txt");
        std::fs::write(&target, b"Synthetic private data").unwrap();
        if let Err(error) = std::os::windows::fs::symlink_file(&target, &link) {
            if error.raw_os_error() == Some(1314) {
                eprintln!("Skipped symlink source check: Windows symlink privilege unavailable");
                return;
            }
            panic!("Cannot create synthetic file link: {error}");
        }
        assert!(matches!(
            open(&link).await,
            Err(Error::UnsupportedCopySource)
        ));
    }

    #[tokio::test]
    async fn native_source_open_reads_relative_absolute_and_extended_paths() {
        use tokio::io::AsyncReadExt;
        let path = crate::sftp::tests::directory().join("source.txt");
        std::fs::write(&path, b"Synthetic source").unwrap();
        for candidate in [
            path.strip_prefix(std::env::current_dir().unwrap())
                .unwrap()
                .to_owned(),
            path.clone(),
            std::fs::canonicalize(&path).unwrap(),
        ] {
            let mut source = open(&candidate).await.unwrap();
            let mut bytes = Vec::new();
            source.read_to_end(&mut bytes).await.unwrap();
            assert_eq!(bytes, b"Synthetic source");
        }
    }

    #[tokio::test]
    async fn native_source_open_rejects_nul_before_path_conversion() {
        assert!(matches!(
            open(Path::new("C:\\source\0hidden.txt")).await,
            Err(Error::InvalidPath)
        ));
    }

    #[tokio::test]
    async fn native_source_open_rejects_oversized_paths_before_native_allocation() {
        assert!(matches!(
            open(Path::new(&"x".repeat(32_761))).await,
            Err(Error::InvalidPath)
        ));
    }

    #[tokio::test]
    async fn upload_handle_rejects_reparse_parent_without_reading_the_target() {
        let root = crate::sftp::tests::directory();
        let target = root.join("private");
        let link = root.join("shared");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("source.txt"), b"Synthetic private data").unwrap();
        if let Err(error) = std::os::windows::fs::symlink_dir(&target, &link) {
            if error.raw_os_error() == Some(1314) {
                eprintln!("Skipped reparse parent check: Windows symlink privilege unavailable");
                return;
            }
            panic!("Cannot create synthetic parent link: {error}");
        }
        assert!(matches!(
            open(&link.join("source.txt")).await,
            Err(Error::UnsupportedCopySource)
        ));
    }
}
