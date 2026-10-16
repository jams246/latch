use std::fs;

use super::TerminalTheme;
use crate::storage::{
    AuthMethod, Connection, SortMode, Store, WindowSize, tests_persistence::store,
};

#[test]
fn older_configuration_defaults_terminal_theme_without_rewriting_file() {
    let store = store("legacy-terminal-theme");
    fs::create_dir_all(store.path.parent().unwrap()).unwrap();
    let original = br#"{"connections":[],"sort_mode":"most_used"}"#;
    fs::write(&store.path, original).unwrap();
    let restored = Store::load(&store.path).unwrap();
    assert_eq!(restored.terminal_theme(), TerminalTheme::Classic);
    assert_eq!(restored.sort_mode(), SortMode::MostUsed);
    assert_eq!(fs::read(&store.path).unwrap(), original);
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
            serde_json::from_slice(&fs::read(&store.path).unwrap()).unwrap();
        assert_eq!(value["terminal_theme"], serialized);
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
fn invalid_terminal_theme_is_rejected_without_rewriting_file() {
    let store = store("invalid-terminal-theme");
    fs::create_dir_all(store.path.parent().unwrap()).unwrap();
    let original = br#"{"terminal_theme":"unknown"}"#;
    fs::write(&store.path, original).unwrap();
    assert_eq!(
        Store::load(&store.path).err().unwrap().kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(fs::read(&store.path).unwrap(), original);
}
