use super::tests::fixture;
use crate::ui::SessionKind;
use gpui::TestAppContext;

#[gpui::test]
fn import_should_wait_until_session_cleanup_finishes(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture.open_settings();
    fixture
        .handle
        .update(&mut fixture.visual, |app, _, cx| {
            app.tabs.clear();
            app.active = None;
            app.pending_cleanup = 1;
            assert!(!app.can_import());
            assert!(app.apply_import(app.store.snapshot(), cx).is_err());
            app.pending_cleanup = 0;
            assert!(app.can_import());
            assert!(!app.preserve_imported_window);
        })
        .unwrap();
}

#[gpui::test]
fn data_operation_should_block_terminal_and_sftp_connections(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            let connection = app.tabs[0].connection.clone();
            app.data_busy = true;
            app.connect(connection.clone(), SessionKind::Terminal, window, cx);
            app.connect(connection, SessionKind::Sftp, window, cx);
            assert_eq!(app.tabs.len(), 1);
            assert_eq!(app.active, Some(0));
            assert!(!app.request_shutdown(cx));
            assert!(!app.shutting_down);
        })
        .unwrap();
}

#[gpui::test]
fn data_operation_should_block_connection_editor(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            app.data_busy = true;
            app.edit(None, window, cx);
            assert!(app.editor.is_none());
        })
        .unwrap();
}

#[gpui::test]
fn data_operation_should_block_connection_delete_dialog(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            let connection = app.tabs[0].connection.clone();
            app.data_busy = true;
            app.request_connection_delete(connection, window, cx);
            assert!(app.connection_delete.is_none());
        })
        .unwrap();
}
