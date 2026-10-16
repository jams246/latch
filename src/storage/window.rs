use std::io;

use serde::{Deserialize, Serialize};

use super::Store;

/// Main-window restore dimensions in logical pixels and its maximized state.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WindowSize {
    /// Width of the window before maximizing.
    pub width: f32,
    /// Height of the window before maximizing.
    pub height: f32,
    /// Whether the window should reopen maximized.
    #[serde(default)]
    pub maximized: bool,
}

impl Default for WindowSize {
    fn default() -> Self {
        Self {
            width: 1120.,
            height: 720.,
            maximized: false,
        }
    }
}

impl WindowSize {
    /// Minimum supported main-window width in logical pixels.
    pub const MIN_WIDTH: f32 = 720.;
    /// Minimum supported main-window height in logical pixels.
    pub const MIN_HEIGHT: f32 = 480.;

    pub(super) fn validate(self) -> io::Result<()> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || self.width < Self::MIN_WIDTH
            || self.height < Self::MIN_HEIGHT
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Window dimensions must be finite and at least 720 by 480 logical pixels",
            ));
        }
        Ok(())
    }
}

impl Store {
    /// Returns the saved main-window size, or the initial size for older settings.
    pub fn window_size(&self) -> WindowSize {
        self.configuration.window_size.unwrap_or_default()
    }

    /// Persists the main-window size without changing connection preferences.
    ///
    /// # Errors
    /// Rejects invalid dimensions and preserves prior state on filesystem failures.
    pub fn set_window_size(&mut self, window_size: WindowSize) -> io::Result<()> {
        window_size.validate()?;
        self.update(|configuration| {
            configuration.window_size = Some(window_size);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::storage::{SortMode, tests_persistence::store};

    #[test]
    fn window_size_and_maximized_state_survive_restart_with_other_preferences() {
        let mut store = store("window-size");
        store.set_sort_mode(SortMode::MostUsed).unwrap();
        for maximized in [false, true] {
            let expected = WindowSize {
                width: 1300.,
                height: 900.,
                maximized,
            };
            store.set_window_size(expected).unwrap();
            let restored = Store::load(&store.path).unwrap();
            assert_eq!(restored.window_size(), expected);
            assert_eq!(restored.sort_mode(), SortMode::MostUsed);
        }
    }

    #[test]
    fn older_configuration_defaults_window_size_without_rewriting_file() {
        let store = store("legacy-window-size");
        fs::create_dir_all(store.path.parent().unwrap()).unwrap();
        let original = br#"{"connections":[],"sort_mode":"most_used"}"#;
        fs::write(&store.path, original).unwrap();
        let restored = Store::load(&store.path).unwrap();
        assert_eq!(restored.window_size(), WindowSize::default());
        assert_eq!(fs::read(&store.path).unwrap(), original);
    }

    #[test]
    fn invalid_dimensions_preserve_saved_size_and_configuration() {
        let mut store = store("invalid-window-size");
        store.set_window_size(WindowSize::default()).unwrap();
        let original = fs::read(&store.path).unwrap();
        for (width, height) in [
            (0., 720.),
            (719., 720.),
            (1120., 479.),
            (f32::NAN, 720.),
            (1120., f32::INFINITY),
        ] {
            let invalid = WindowSize {
                width,
                height,
                maximized: false,
            };
            assert!(store.set_window_size(invalid).is_err());
        }
        assert_eq!(store.window_size(), WindowSize::default());
        assert_eq!(fs::read(&store.path).unwrap(), original);
    }

    #[test]
    fn invalid_configured_dimensions_are_rejected_without_rewriting_file() {
        let store = store("invalid-configured-window-size");
        fs::create_dir_all(store.path.parent().unwrap()).unwrap();
        let original = br#"{"window_size":{"width":1,"height":720}}"#;
        fs::write(&store.path, original).unwrap();
        assert_eq!(
            Store::load(&store.path).err().unwrap().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(&store.path).unwrap(), original);
    }

    #[test]
    fn failed_save_preserves_previous_window_size() {
        let mut store = store("failed-window-size");
        fs::create_dir_all(&store.path).unwrap();
        let changed = WindowSize {
            width: 1400.,
            ..WindowSize::default()
        };
        assert!(store.set_window_size(changed).is_err());
        assert_eq!(store.window_size(), WindowSize::default());
    }
}
