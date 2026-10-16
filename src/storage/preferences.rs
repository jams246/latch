#[cfg(test)]
mod tests;

use std::io;

use serde::{Deserialize, Serialize};

use super::Store;

/// Remembered terminal color preset, independent of application chrome.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalTheme {
    /// Original terminal colors.
    #[default]
    Classic,
    /// Near-black surfaces with muted, balanced ANSI colors.
    Carbon,
    /// Cool blue-gray surfaces with soft ANSI colors.
    Glacier,
    /// Warm charcoal surfaces with warm ANSI colors.
    Ember,
    /// Deep navy surfaces with vivid ANSI colors.
    Midnight,
}

impl Store {
    /// Returns the saved terminal theme.
    pub fn terminal_theme(&self) -> TerminalTheme {
        self.configuration.terminal_theme
    }

    /// Persists the terminal theme without changing other preferences.
    ///
    /// # Errors
    /// Returns filesystem failures, leaving the prior preference unchanged.
    pub fn set_terminal_theme(&mut self, terminal_theme: TerminalTheme) -> io::Result<()> {
        self.update(|configuration| {
            configuration.terminal_theme = terminal_theme;
            Ok(())
        })
    }
}
