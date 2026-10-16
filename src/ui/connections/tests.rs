use crate::ui::Latch;
use gpui::{KeyUpEvent, Keystroke, TestAppContext, VisualTestContext, WindowHandle};
use latch::storage::{AuthMethod, Connection, Store};
use std::{path::PathBuf, sync::Arc};

fn open_editor(cx: &mut TestAppContext, connection: Option<Connection>) -> WindowHandle<Latch> {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/app-updates/editor")
        .join(uuid::Uuid::new_v4().to_string());
    let store = Store::load(directory.join("latch.db")).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    handle
        .update(cx, |app, window, cx| app.edit(connection, window, cx))
        .unwrap();
    handle
}

fn key_connection() -> Connection {
    Connection::new(
        "Synthetic".into(),
        "example.invalid".into(),
        "tester".into(),
        AuthMethod::KeyFile {
            path: "existing-key.ppk".into(),
        },
    )
}

#[gpui::test]
fn editor_should_accept_typing_and_save_after_keyboard_navigation(cx: &mut TestAppContext) {
    let handle = open_editor(cx, None);
    cx.run_until_parked();
    cx.simulate_input(*handle, "Synthetic");
    cx.simulate_keystrokes(*handle, "tab");
    cx.simulate_input(*handle, "example.invalid");
    cx.simulate_keystrokes(*handle, "tab tab");
    cx.simulate_input(*handle, "tester");
    cx.simulate_keystrokes(*handle, "enter");
    handle
        .read_with(cx, |app, _| {
            assert!(app.editor.is_some());
            assert!(app.store.connections().is_empty());
        })
        .unwrap();
    let cx = VisualTestContext::from_window(*handle, cx).into_mut();
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    crate::ui::persistence::tests::settle(handle, cx);
    handle
        .read_with(cx, |app, _| {
            assert!(app.editor.is_none());
            let connection = &app.store.connections()[0];
            assert_eq!(connection.name, "Synthetic");
            assert_eq!(connection.host, "example.invalid");
            assert_eq!(connection.port, 22);
            assert_eq!(connection.username, "tester");
        })
        .unwrap();
}

#[gpui::test]
fn chosen_private_key_should_replace_displayed_path_and_save(cx: &mut TestAppContext) {
    let handle = open_editor(cx, Some(key_connection()));
    let selected = PathBuf::from(r"C:\Synthetic Keys\clé_test.ppk");
    handle
        .update(cx, |app, window, cx| {
            let field = app.editor.as_ref().unwrap().fields[4].entity_id();
            app.apply_key_selection(field, Ok(Some(vec![selected.clone()])), cx);
            let input = app.editor.as_ref().unwrap().fields[4].read(cx);
            assert_eq!(input.value.as_str(), selected.to_str().unwrap());
            assert_eq!(input.selection, input.value.len()..input.value.len());
            app.save_connection(window, cx);
        })
        .unwrap();
    crate::ui::persistence::tests::settle(handle, cx);
    handle
        .read_with(cx, |app, _| {
            assert_eq!(
                app.store.connections()[0].auth,
                AuthMethod::KeyFile { path: selected }
            );
        })
        .unwrap();
}

#[gpui::test]
fn cancelled_private_key_picker_should_preserve_existing_path(cx: &mut TestAppContext) {
    let handle = open_editor(cx, Some(key_connection()));
    handle
        .update(cx, |app, _, cx| {
            let field = app.editor.as_ref().unwrap().fields[4].entity_id();
            app.apply_key_selection(field, Ok(None), cx);
            assert_eq!(
                app.editor.as_ref().unwrap().fields[4]
                    .read(cx)
                    .value
                    .as_str(),
                "existing-key.ppk"
            );
            assert!(app.error.is_none());
        })
        .unwrap();
}

#[gpui::test]
fn failed_private_key_picker_should_preserve_existing_path(cx: &mut TestAppContext) {
    let handle = open_editor(cx, Some(key_connection()));
    handle
        .update(cx, |app, _, cx| {
            let field = app.editor.as_ref().unwrap().fields[4].entity_id();
            app.apply_key_selection(field, Err(anyhow::anyhow!("synthetic failure")), cx);
            assert_eq!(
                app.editor.as_ref().unwrap().fields[4]
                    .read(cx)
                    .value
                    .as_str(),
                "existing-key.ppk"
            );
            assert_eq!(
                app.error.as_deref(),
                Some("Cannot choose private-key file: synthetic failure")
            );
        })
        .unwrap();
}

#[gpui::test]
fn stale_private_key_picker_should_ignore_closed_and_reopened_editor(cx: &mut TestAppContext) {
    let handle = open_editor(cx, Some(key_connection()));
    handle
        .update(cx, |app, window, cx| {
            let old_field = app.editor.as_ref().unwrap().fields[4].entity_id();
            app.close_editor(window, cx);
            app.apply_key_selection(old_field, Ok(Some(vec!["stale-key.ppk".into()])), cx);
            assert!(app.editor.is_none());
            app.edit(Some(key_connection()), window, cx);
            app.apply_key_selection(old_field, Ok(Some(vec!["stale-key.ppk".into()])), cx);
            app.apply_key_selection(old_field, Err(anyhow::anyhow!("stale failure")), cx);
            assert_eq!(
                app.editor.as_ref().unwrap().fields[4]
                    .read(cx)
                    .value
                    .as_str(),
                "existing-key.ppk"
            );
            assert!(app.error.is_none());
        })
        .unwrap();
}

#[gpui::test]
fn pending_save_blocks_typing_focus_changes_and_late_key_picker(cx: &mut TestAppContext) {
    let handle = open_editor(cx, Some(key_connection()));
    let (release, wait) = std::sync::mpsc::channel();
    handle
        .update(cx, |app, window, cx| {
            app.save_preference(
                "barrier",
                move |_| {
                    wait.recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    Ok(())
                },
                cx,
            );
            app.save_connection(window, cx);
        })
        .unwrap();
    cx.simulate_input(*handle, "ignored edit");
    cx.simulate_keystrokes(*handle, "tab");
    handle
        .update(cx, |app, window, cx| {
            let field = app.editor.as_ref().unwrap().fields[4].entity_id();
            app.apply_key_selection(field, Ok(Some(vec!["ignored.ppk".into()])), cx);
            let editor = app.editor.as_ref().unwrap();
            assert!(editor.focus.is_focused(window));
            assert_eq!(editor.fields[0].read(cx).value.as_str(), "Synthetic");
            assert_eq!(editor.fields[4].read(cx).value.as_str(), "existing-key.ppk");
        })
        .unwrap();
    release.send(()).unwrap();
    crate::ui::persistence::tests::settle(handle, cx);
    handle
        .read_with(cx, |app, _| {
            assert!(app.editor.is_none());
            assert_eq!(app.store.connections()[0].name, "Synthetic");
        })
        .unwrap();
}
