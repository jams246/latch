use std::fs;

use super::TerminalTheme;
use crate::storage::{
    AuthMethod, Connection, SortMode, Store, WindowSize, tests_persistence::store,
};

#[test]
fn fresh_settings_default_to_classic_terminal_theme() {
    let store = store("default-terminal-theme");
    assert_eq!(store.terminal_theme(), TerminalTheme::Classic);
    assert!(!store.path.exists());
}

#[test]
fn all_terminal_themes_survive_restart_with_other_preferences() {
    let mut store = store("terminal-theme-restart");
    let mut connection = Connection::new(
        "Synthetic server".into(),
        "example.invalid".into(),
        "tester".into(),
        AuthMethod::Password,
    );
    store.upsert(connection.clone()).unwrap();
    store.record_successful_shell(&connection.id).unwrap();
    connection.successful_shells = 1;
    store.set_sort_mode(SortMode::FrequentAndName).unwrap();
    let window_size = WindowSize {
        width: 1280.,
        height: 800.,
        maximized: true,
    };
    store.set_window_size(window_size).unwrap();
    for (theme, serialized) in [
        (TerminalTheme::Classic, "classic"),
        (TerminalTheme::Carbon, "carbon"),
        (TerminalTheme::Glacier, "glacier"),
        (TerminalTheme::Ember, "ember"),
        (TerminalTheme::Midnight, "midnight"),
    ] {
        store.set_terminal_theme(theme).unwrap();
        let restored = Store::load(&store.path).unwrap();
        assert_eq!(restored.terminal_theme(), theme);
        assert_eq!(restored.connections(), std::slice::from_ref(&connection));
        assert_eq!(restored.sort_mode(), SortMode::FrequentAndName);
        assert_eq!(restored.window_size(), window_size);
        let value: serde_json::Value =
            serde_json::from_slice(&store.export_json().unwrap()).unwrap();
        assert_eq!(value["configuration"]["terminal_theme"], serialized);
    }
}

#[test]
fn failed_save_preserves_previous_terminal_theme() {
    let mut store = store("failed-terminal-theme");
    fs::create_dir_all(&store.path).unwrap();
    assert!(store.set_terminal_theme(TerminalTheme::Carbon).is_err());
    assert_eq!(store.terminal_theme(), TerminalTheme::Classic);
    assert_eq!(
        fs::read_dir(store.path.parent().unwrap()).unwrap().count(),
        1
    );
}

#[cfg(windows)]
#[test]
fn locked_configuration_preserves_saved_terminal_theme_and_file() {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};

    let mut store = store("locked-terminal-theme");
    store.set_terminal_theme(TerminalTheme::Carbon).unwrap();
    let original = fs::read(&store.path).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&store.path)
        .unwrap();
    assert!(store.set_terminal_theme(TerminalTheme::Midnight).is_err());
    assert_eq!(store.terminal_theme(), TerminalTheme::Carbon);
    drop(lock);
    assert_eq!(fs::read(&store.path).unwrap(), original);
}

#[test]
fn invalid_imported_terminal_theme_is_rejected_without_changing_settings() {
    let mut store = store("invalid-terminal-theme");
    store.set_terminal_theme(TerminalTheme::Carbon).unwrap();
    let original = fs::read(&store.path).unwrap();
    let mut snapshot: serde_json::Value =
        serde_json::from_slice(&store.export_json().unwrap()).unwrap();
    snapshot["configuration"]["terminal_theme"] = "unknown".into();
    assert_eq!(
        Store::parse_import(&serde_json::to_vec(&snapshot).unwrap())
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(store.terminal_theme(), TerminalTheme::Carbon);
    assert_eq!(fs::read(&store.path).unwrap(), original);
}
