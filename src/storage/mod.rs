//! Non-secret connection metadata, durable preferences, and deterministic picker sorting.

mod model;
mod persistence;
mod portable;
mod preferences;
mod window;

#[cfg(test)]
mod tests_persistence;
#[cfg(test)]
mod tests_portable;
#[cfg(test)]
mod tests_sorting;

use std::{cmp::Ordering, collections::HashSet, io, path::PathBuf};

use serde::{Deserialize, Serialize};

pub use model::{AuthMethod, Connection, SortMode};
pub use preferences::TerminalTheme;
pub use window::WindowSize;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Configuration {
    connections: Vec<Connection>,
    sort_mode: SortMode,
    #[serde(default)]
    terminal_theme: TerminalTheme,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_size: Option<WindowSize>,
}

/// Connection metadata persisted as one atomically replaced JSON file.
pub struct Store {
    path: PathBuf,
    configuration: Configuration,
}

impl Store {
    /// Loads a store, returning an empty store only when the file does not exist.
    ///
    /// # Errors
    /// Invalid or unreadable configuration is reported without overwriting it.
    pub fn load(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let configuration = persistence::read(&path)?;
        validate(&configuration)?;
        Ok(Self {
            path,
            configuration,
        })
    }

    /// Writes the current metadata without modifying the prior file on failure.
    ///
    /// # Errors
    /// Returns filesystem and serialization failures.
    pub fn save(&self) -> io::Result<()> {
        persistence::write(&self.path, &self.configuration)
    }

    /// Returns all saved connections in storage order.
    pub fn connections(&self) -> &[Connection] {
        &self.configuration.connections
    }

    /// Returns the persisted picker sorting preference.
    pub fn sort_mode(&self) -> SortMode {
        self.configuration.sort_mode
    }

    /// Changes and persists the sorting preference without touching sessions.
    ///
    /// # Errors
    /// Returns filesystem failures, leaving the prior preference unchanged.
    pub fn set_sort_mode(&mut self, sort_mode: SortMode) -> io::Result<()> {
        self.update(|configuration| {
            configuration.sort_mode = sort_mode;
            Ok(())
        })
    }

    /// Adds or edits a connection while preserving the stored usage count.
    ///
    /// # Errors
    /// Rejects invalid metadata and preserves prior state on filesystem failures.
    pub fn upsert(&mut self, mut connection: Connection) -> io::Result<()> {
        connection.validate()?;
        self.update(|configuration| {
            if let Some(existing) = configuration
                .connections
                .iter_mut()
                .find(|item| item.id == connection.id)
            {
                connection.successful_shells = existing.successful_shells;
                *existing = connection;
            } else {
                connection.successful_shells = 0;
                configuration.connections.push(connection);
            }
            Ok(())
        })
    }

    /// Deletes a connection together with its usage statistics.
    ///
    /// # Errors
    /// Returns filesystem failures, leaving prior state unchanged.
    pub fn delete(&mut self, id: &str) -> io::Result<()> {
        self.update(|configuration| {
            configuration.connections.retain(|item| item.id != id);
            Ok(())
        })
    }

    /// Records one successfully opened, user-initiated remote terminal shell.
    /// Call once after authentication and shell startup, never for SFTP or retries.
    ///
    /// # Errors
    /// Returns missing-ID or filesystem errors without changing the stored count.
    pub fn record_successful_shell(&mut self, id: &str) -> io::Result<()> {
        self.update(|configuration| {
            let connection = configuration
                .connections
                .iter_mut()
                .find(|item| item.id == id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Connection was deleted"))?;
            connection.successful_shells = connection.successful_shells.saturating_add(1);
            Ok(())
        })
    }

    /// Returns the complete list in the selected order, including never-used entries.
    /// For Frequent + Name, render [`Self::frequent`] before this alphabetical list.
    pub fn sorted(&self) -> Vec<&Connection> {
        let mut connections: Vec<_> = self.connections().iter().collect();
        if self.sort_mode() == SortMode::MostUsed {
            connections.sort_by(|a, b| compare_usage(a, b));
        } else {
            connections.sort_by(|a, b| compare_names(a, b));
        }
        connections
    }

    /// Returns up to three most-used connections; duplicates in the full list remain.
    pub fn frequent(&self) -> Vec<&Connection> {
        let mut connections: Vec<_> = self.connections().iter().collect();
        connections.sort_by(|a, b| compare_usage(a, b));
        connections.truncate(3);
        connections
    }

    fn update(
        &mut self,
        change: impl FnOnce(&mut Configuration) -> io::Result<()>,
    ) -> io::Result<()> {
        let mut next = self.configuration.clone();
        change(&mut next)?;
        persistence::write(&self.path, &next)?;
        self.configuration = next;
        Ok(())
    }
}

fn compare_names(a: &Connection, b: &Connection) -> Ordering {
    a.name
        .to_lowercase()
        .cmp(&b.name.to_lowercase())
        .then_with(|| a.id.cmp(&b.id))
}

fn compare_usage(a: &Connection, b: &Connection) -> Ordering {
    b.successful_shells
        .cmp(&a.successful_shells)
        .then_with(|| compare_names(a, b))
}

fn validate(configuration: &Configuration) -> io::Result<()> {
    if let Some(window_size) = configuration.window_size {
        window_size.validate()?;
    }
    let mut ids = HashSet::new();
    for connection in &configuration.connections {
        connection.validate()?;
        if !ids.insert(&connection.id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Connection configuration contains duplicate stable IDs",
            ));
        }
    }
    Ok(())
}
