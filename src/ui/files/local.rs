#[cfg(windows)]
mod native;
mod operations;
#[cfg(test)]
mod persistence_tests;
#[cfg(test)]
mod tests;

pub(in crate::ui) use operations::{create_directory, delete, rename, validate_name};

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
};

pub(in crate::ui) struct Entry {
    pub metadata: latch::sftp::Entry,
    pub path: PathBuf,
    pub icon: Option<Arc<gpui::RenderImage>>,
}

pub(in crate::ui) struct Directory {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub drives: Vec<PathBuf>,
}

pub(super) fn list_initial(initial_directory: Option<PathBuf>) -> io::Result<Directory> {
    let mut candidates: Vec<_> = initial_directory.into_iter().collect();
    candidates.extend(home_directory());
    candidates.extend(std::env::current_dir().ok());
    #[cfg(windows)]
    candidates.extend(native::drives());
    candidates.push(PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" }));
    list_first_available(candidates)
}

fn list_first_available(candidates: Vec<PathBuf>) -> io::Result<Directory> {
    let mut seen = std::collections::HashSet::new();
    let mut error = io::Error::new(io::ErrorKind::NotFound, "No local directory is available");
    for path in candidates
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
    {
        match list(&path) {
            Ok(directory) => return Ok(directory),
            Err(failure) => error = failure,
        }
    }
    Err(error)
}

pub(super) fn list(path: &Path) -> io::Result<Directory> {
    #[cfg(windows)]
    let _apartment = native::Apartment::new()?;
    let path = canonicalize(path)?;
    let mut entries = Vec::new();
    for item in fs::read_dir(&path)? {
        let item = item?;
        let path = item.path();
        let metadata = match item.metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let is_link = metadata.file_type().is_symlink() || is_shortcut(&path);
        let is_dir = metadata.is_dir()
            || (metadata.file_type().is_symlink()
                && fs::metadata(&path).is_ok_and(|target| target.is_dir()));
        #[cfg(windows)]
        let icon = native::icon(&path, is_dir, is_link);
        #[cfg(not(windows))]
        let icon = None;
        entries.push(Entry {
            metadata: latch::sftp::Entry {
                name: item.file_name().to_string_lossy().into_owned(),
                path: path.to_string_lossy().into_owned(),
                is_dir,
                is_link,
                modified: metadata
                    .modified()
                    .ok()
                    .and_then(|date| date.duration_since(UNIX_EPOCH).ok())
                    .map(|date| date.as_secs()),
                size: (!is_dir).then_some(metadata.len()),
            },
            path,
            icon,
        });
    }
    #[cfg(windows)]
    let drives = native::drives();
    #[cfg(not(windows))]
    let drives = vec![PathBuf::from("/")];
    Ok(Directory {
        path,
        entries,
        drives,
    })
}

fn home_directory() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
}

pub(super) fn follow_link(path: &Path) -> io::Result<PathBuf> {
    #[cfg(windows)]
    let target = if is_shortcut(path) {
        native::resolve_shortcut(path)?
    } else {
        path.to_path_buf()
    };
    #[cfg(not(windows))]
    let target = path.to_path_buf();
    let target = canonicalize(&target)?;
    if fs::metadata(&target)?.is_dir() {
        Ok(target)
    } else {
        target
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| io::Error::other("The link target has no parent directory"))
    }
}

fn is_shortcut(path: &Path) -> bool {
    cfg!(windows)
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
}

fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    let canonical = fs::canonicalize(path)?;
    #[cfg(windows)]
    let canonical = native::display_path(&canonical);
    Ok(canonical)
}
