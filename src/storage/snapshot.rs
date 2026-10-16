use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, Visitor},
};

use super::{
    Configuration, Connection, SortMode, Store, TerminalTheme, WindowSize, files, locking,
    persistence, portable, recovery, validate,
};

/// A validated, complete settings backup ready to replace a profile.
#[derive(Clone)]
pub struct ImportSnapshot(Configuration);

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    format_version: u32,
    configuration: SnapshotConfiguration,
    #[serde(deserialize_with = "unique_directories")]
    local_directories: BTreeMap<String, PathBuf>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SnapshotConfiguration {
    connections: Vec<Connection>,
    sort_mode: SortMode,
    terminal_theme: TerminalTheme,
    window_size: Option<WindowSize>,
}

impl Store {
    /// Maximum size accepted for a plaintext JSON settings backup.
    pub const MAX_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;

    /// Parses a version 1 backup without modifying the current profile.
    ///
    /// # Errors
    /// Rejects oversized, unsupported, malformed, or invalid backups.
    pub fn parse_import(bytes: &[u8]) -> io::Result<ImportSnapshot> {
        if bytes.len() > Self::MAX_SNAPSHOT_BYTES {
            return Err(invalid_backup());
        }
        let snapshot: Snapshot = serde_json::from_slice(bytes).map_err(|_| invalid_backup())?;
        if snapshot.format_version != 1 {
            return Err(invalid_backup());
        }
        let configuration = Configuration {
            connections: snapshot.configuration.connections,
            sort_mode: snapshot.configuration.sort_mode,
            terminal_theme: snapshot.configuration.terminal_theme,
            window_size: snapshot.configuration.window_size,
            local_directories: snapshot.local_directories,
        };
        validate(&configuration)?;
        Ok(ImportSnapshot(configuration))
    }

    /// Captures immutable metadata for serialization on a worker thread.
    pub fn snapshot(&self) -> ImportSnapshot {
        ImportSnapshot(self.configuration.clone())
    }

    /// Completely replaces saved metadata, committing before changing memory.
    ///
    /// # Errors
    /// Database failures preserve the current settings and connections.
    pub fn replace_import(&mut self, snapshot: ImportSnapshot) -> io::Result<()> {
        persistence::write(&self.path, &snapshot.0)?;
        self.configuration = snapshot.0;
        Ok(())
    }

    /// Reads and validates a bounded plaintext JSON backup.
    ///
    /// # Errors
    /// Returns filesystem errors or invalid-backup errors without changing data.
    pub fn read_import_file(path: &Path) -> io::Result<ImportSnapshot> {
        Self::parse_import(&files::read_limited(path, Self::MAX_SNAPSHOT_BYTES)?)
    }

    /// Atomically exports JSON while protecting the profile's internal files.
    ///
    /// # Errors
    /// Rejects protected targets and preserves existing output on failed writes.
    pub fn write_export(directory: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let root = std::fs::canonicalize(directory)?;
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = path.file_name().ok_or_else(invalid_backup)?;
        let requested = std::fs::canonicalize(parent)?.join(name);
        let target = std::fs::canonicalize(path).or_else(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                Ok(requested.clone())
            } else {
                Err(error)
            }
        })?;
        if protected(&root, &requested)
            || protected(&root, &target)
            || bytes.len() > Self::MAX_SNAPSHOT_BYTES
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Choose a backup file outside Latch's internal profile files",
            ));
        }
        files::atomic_write(path, bytes)
    }
}

impl ImportSnapshot {
    pub(super) fn into_configuration(self) -> Configuration {
        self.0
    }

    /// Serializes this immutable snapshot as a plaintext JSON backup.
    ///
    /// # Errors
    /// Returns serialization or oversized-backup errors.
    pub fn export_json(&self) -> io::Result<Vec<u8>> {
        encode(&self.0)
    }
}

pub(super) fn validate_size(configuration: &Configuration) -> io::Result<()> {
    encode(configuration).map(|_| ())
}

fn encode(configuration: &Configuration) -> io::Result<Vec<u8>> {
    let snapshot = Snapshot {
        format_version: 1,
        configuration: SnapshotConfiguration {
            connections: configuration.connections.clone(),
            sort_mode: configuration.sort_mode,
            terminal_theme: configuration.terminal_theme,
            window_size: configuration.window_size,
        },
        local_directories: configuration.local_directories.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(io::Error::other)?;
    if bytes.len() > Store::MAX_SNAPSHOT_BYTES {
        return Err(invalid_backup());
    }
    Ok(bytes)
}

fn protected(root: &Path, path: &Path) -> bool {
    let root = root.to_string_lossy().to_lowercase();
    let path = path.to_string_lossy().to_lowercase();
    let prefix = format!(
        "{}{separator}",
        root.trim_end_matches(['/', '\\']),
        separator = std::path::MAIN_SEPARATOR
    );
    let Some(relative) = path.strip_prefix(&prefix) else {
        return false;
    };
    let first = relative
        .split(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.']);
    first.starts_with("latch-stage-")
        || [
            portable::DATABASE,
            "key.dpapi",
            "latch.db-journal",
            "latch.db-wal",
            "latch.db-shm",
        ]
        .into_iter()
        .any(|name| first == name || first.starts_with(&format!("{name}.archive-")))
        || first == locking::LOCK_FILE
        || first == recovery::MARKER
        || first == "known_hosts"
}

fn invalid_backup() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Settings backup is invalid, unsupported, or larger than 16 MiB",
    )
}

pub(super) fn unique_directories<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, PathBuf>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Directories;
    impl<'de> Visitor<'de> for Directories {
        type Value = BTreeMap<String, PathBuf>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("unique connection directory preferences")
        }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut directories = BTreeMap::new();
            while let Some((id, path)) = map.next_entry::<String, PathBuf>()? {
                if directories.insert(id, path).is_some() {
                    return Err(de::Error::custom(
                        "duplicate connection directory preference",
                    ));
                }
            }
            Ok(directories)
        }
    }
    deserializer.deserialize_map(Directories)
}
