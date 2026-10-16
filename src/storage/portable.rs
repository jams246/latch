use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::{Configuration, Store, files, locking, pair, recovery, validate};

pub(super) const LEGACY_DIRECTORY: &str = "latch-data";
pub(super) const DATABASE: &str = "latch.db";

impl Store {
    /// Loads an encrypted portable profile, importing legacy JSON only once.
    /// Existing encrypted profiles always take precedence, including damaged ones.
    ///
    /// # Errors
    /// Reports locked, damaged, or unreadable profiles without resetting them.
    pub fn load_portable(directory: &Path, roaming_directory: Option<&Path>) -> io::Result<Self> {
        let lock = locking::acquire(directory)?;
        if files::exists(&directory.join(recovery::MARKER))? {
            return Err(recovery::interrupted());
        }
        let mut import_warnings = Vec::new();
        if !pair::present(directory)? {
            let old = directory.join(LEGACY_DIRECTORY);
            if files::exists(&old)? {
                pair::require(&old)?;
                let configuration = Self::load(old.join(DATABASE))?.configuration;
                recovery::publish(&old, directory, &configuration)?;
            } else {
                if pair::leftovers(directory)? {
                    return Err(recovery::interrupted());
                }
                let (configuration, source, warnings) =
                    legacy_configuration(directory, roaming_directory)?;
                import_warnings = warnings;
                let stage = recovery::stage(directory, &configuration)?;
                if let Some(source) = source {
                    copy_host_trust(&source, directory)?;
                }
                recovery::publish(&stage, directory, &configuration)?;
            }
        }
        pair::require(directory)?;
        let mut store = Self::load(directory.join(DATABASE))?;
        store._lock = Some(lock);
        store.warnings = legacy_warnings(directory, roaming_directory);
        store.warnings.extend(import_warnings);
        Ok(store)
    }
}

fn legacy_configuration(
    directory: &Path,
    roaming: Option<&Path>,
) -> io::Result<(Configuration, Option<PathBuf>, Vec<String>)> {
    let candidates = [
        Some(directory.join("latch.json")),
        Some(directory.join("connections.json")),
        roaming.map(|path| path.join("connections.json")),
    ];
    for path in candidates.into_iter().flatten() {
        if !files::exists(&path)? {
            continue;
        }
        let bytes = files::read_limited(&path, Store::MAX_SNAPSHOT_BYTES)?;
        let mut configuration: Configuration = serde_json::from_slice(&bytes).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Legacy settings are invalid; the original file was preserved",
            )
        })?;
        validate(&configuration)?;
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Legacy settings have no directory"))?;
        let mut warnings = Vec::new();
        for connection in &configuration.connections {
            let preference = preference_path(parent, &connection.id);
            let remembered: io::Result<Option<PathBuf>> = (|| {
                if !files::exists(&preference)? {
                    return Ok(None);
                }
                let bytes = files::read_limited(&preference, Store::MAX_SNAPSHOT_BYTES)?;
                let path: PathBuf = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                super::local_directories::validate_path(&path)?;
                Ok(Some(path))
            })();
            match remembered {
                Ok(Some(path)) => { configuration.local_directories.insert(connection.id.clone(), path); }
                Ok(None) => {}
                Err(_) => warnings.push(format!("Cannot import local directory preference at {}. The original file was preserved.", preference.display())),
            }
        }
        validate(&configuration)?;
        return Ok((configuration, Some(parent.to_path_buf()), warnings));
    }
    Ok((Configuration::default(), None, Vec::new()))
}

fn preference_path(directory: &Path, id: &str) -> PathBuf {
    use std::fmt::Write;
    let mut name = String::new();
    for byte in id.bytes() {
        let _ = write!(name, "{byte:02x}");
    }
    directory
        .join("sftp-local-directories")
        .join(format!("{name}.json"))
}

pub(super) fn legacy_warnings(directory: &Path, roaming: Option<&Path>) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for directory in [Some(directory), roaming].into_iter().flatten() {
        for name in [
            "latch.json",
            "connections.json",
            "sftp-local-directories",
            "sftp-local-directory.json",
        ] {
            let path = directory.join(name);
            if path.exists() && seen.insert(path.clone()) {
                warnings.push(format!("Unencrypted legacy backup retained at {}. It is no longer used; remove it after verifying your new profile and exported backup.", path.display()));
            }
        }
    }
    warnings
}

fn copy_host_trust(source: &Path, destination: &Path) -> io::Result<()> {
    let destination = destination.join("known_hosts");
    if files::exists(&destination)? {
        return Ok(());
    }
    let source = source.join("known_hosts");
    if files::exists(&source)? {
        files::create_new(&destination, &fs::read(source)?)?;
    }
    Ok(())
}
