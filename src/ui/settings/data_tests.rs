use super::tests::fixture;
use gpui::TestAppContext;
use latch::{storage::Store, terminal::TerminalPalette};

#[gpui::test]
fn export_warning_should_cancel_without_writing_and_release_busy_state(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture.open_settings();
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            app.export_data(window, cx);
            assert!(app.data_busy);
        })
        .unwrap();
    let (message, detail) = cx.pending_prompt().unwrap();
    assert!(message.contains("unencrypted"));
    assert!(detail.contains("Passwords and private keys are never included"));
    cx.simulate_prompt_answer("Cancel");
    fixture.visual.run_until_parked();
    fixture
        .handle
        .read_with(&fixture.visual, |app, _| {
            assert!(!app.data_busy);
            assert!(app.settings.as_ref().unwrap().error.is_none());
        })
        .unwrap();
}

#[gpui::test]
fn import_should_reject_active_tabs_without_changing_theme(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture.open_settings();
    fixture
        .handle
        .update(&mut fixture.visual, |app, _, cx| {
            let snapshot = Store::parse_import(&app.store.export_json().unwrap()).unwrap();
            assert!(app.apply_import(snapshot, cx).is_err());
            assert!(!app.preserve_imported_window);
            assert_eq!(app.store.terminal_theme(), TerminalPalette::Classic);
        })
        .unwrap();
}

#[gpui::test]
fn import_should_apply_theme_clear_search_and_preserve_imported_window(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture.open_settings();
    fixture
        .handle
        .update(&mut fixture.visual, |app, _, cx| {
            app.tabs.clear();
            app.active = None;
            app.store
                .set_terminal_theme(TerminalPalette::Carbon)
                .unwrap();
            let snapshot = Store::parse_import(&app.store.export_json().unwrap()).unwrap();
            app.store
                .set_terminal_theme(TerminalPalette::Classic)
                .unwrap();
            app.apply_import(snapshot, cx).unwrap();
        })
        .unwrap();
    crate::ui::persistence::tests::settle(fixture.handle, &mut fixture.visual);
    fixture
        .handle
        .read_with(&fixture.visual, |app, cx| {
            assert_eq!(TerminalPalette::current(cx), TerminalPalette::Carbon);
            assert!(app.connection_search.read(cx).value.is_empty());
            assert!(app.preserve_imported_window);
        })
        .unwrap();
}

#[gpui::test]
fn data_operation_should_block_theme_changes_and_settings_close(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture.open_settings();
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            app.data_busy = true;
            app.choose_terminal_theme(TerminalPalette::Carbon, cx);
            app.close_settings(window, cx);
            assert!(app.settings.is_some());
            assert_eq!(app.store.terminal_theme(), TerminalPalette::Classic);
        })
        .unwrap();
}
