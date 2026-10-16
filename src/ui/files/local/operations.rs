use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

#[cfg(test)]
mod tests;

pub(in crate::ui) fn rename(path: &Path, name: &str) -> io::Result<()> {
    validate_name(name)?;
    let source = target_path(path)?;
    if source.file_name().is_some_and(|current| current == name) {
        return fs::symlink_metadata(source).map(|_| ());
    }
    let destination = source.with_file_name(name);
    rename_without_replace(&source, &destination)
}

pub(in crate::ui) fn create_directory(parent: &Path, name: &str) -> io::Result<()> {
    validate_name(name)?;
    fs::create_dir(parent.join(name))
}

pub(in crate::ui) fn delete(paths: &[PathBuf]) -> io::Result<()> {
    let targets: Vec<_> = paths
        .iter()
        .map(|path| {
            let target = target_path(path)?;
            fs::symlink_metadata(&target)?;
            Ok(target)
        })
        .collect::<io::Result<_>>()?;
    for path in targets {
        remove(&path)?;
    }
    Ok(())
}

pub(in crate::ui) fn validate_name(name: &str) -> io::Result<()> {
    let invalid = name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character));
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(suffix.as_bytes(), [b'1'..=b'9']) || matches!(suffix, "¹" | "²" | "³")
        })
    });
    if invalid || reserved {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Enter a valid file or folder name without a path",
        ));
    }
    Ok(())
}

fn target_path(path: &Path) -> io::Result<PathBuf> {
    let tail = path
        .as_os_str()
        .as_encoded_bytes()
        .rsplit(|byte| matches!(byte, b'/' | b'\\'))
        .find(|part| !part.is_empty());
    if !path.is_absolute()
        || tail.is_some_and(|part| part == b"." || part == b"..")
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid_target());
    }
    let name = path.file_name().ok_or_else(invalid_target)?;
    let parent = path.parent().ok_or_else(invalid_target)?;
    Ok(fs::canonicalize(parent)?.join(name))
}

fn invalid_target() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "Select a file or folder, not a filesystem root or parent directory",
    )
}

#[cfg(windows)]
fn rename_without_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use windows::{Win32::Storage::FileSystem::MoveFileW, core::PCWSTR};
    let source = super::native::wide(source)?;
    let destination = super::native::wide(destination)?;
    // MoveFileW fails if the destination exists, including a concurrently created file.
    unsafe { MoveFileW(PCWSTR(source.as_ptr()), PCWSTR(destination.as_ptr())) }
        .map_err(|error| io::Error::from_raw_os_error(error.code().0 & 0xffff))
}

#[cfg(not(windows))]
fn rename_without_replace(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Renaming without replacing existing files is unavailable on this platform",
    ))
}

fn remove(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        };
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
            return if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY.0 != 0 {
                fs::remove_dir(path)
            } else {
                fs::remove_file(path)
            };
        }
    }
    if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}
