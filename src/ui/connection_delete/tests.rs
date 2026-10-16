use crate::ui::Latch;
use gpui::{
    IntoElement, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext, WindowHandle, point, px,
    size,
};
use latch::storage::{AuthMethod, Connection, SortMode, Store};
use std::{fs, path::PathBuf, sync::Arc};

fn fixture(
    cx: &mut TestAppContext,
) -> (
    WindowHandle<Latch>,
    VisualTestContext,
    [Connection; 2],
    PathBuf,
) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/connection-delete/tests")
        .join(uuid::Uuid::new_v4().to_string());
    let path = directory.join("latch.json");
    let mut store = Store::load(&path).unwrap();
    let connections =
        [("first", "First synthetic"), ("second", "Second synthetic")].map(|(id, name)| {
            let mut connection = Connection::new(
                name.into(),
                "example.invalid".into(),
                "tester".into(),
                AuthMethod::Password,
            );
            connection.id = id.into();
            connection
        });
    for connection in &connections {
        store.upsert(connection.clone()).unwrap();
    }
    store.set_sort_mode(SortMode::FrequentAndName).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    let mut visual = VisualTestContext::from_window(*handle, cx);
    visual.simulate_resize(size(px(1000.), px(800.)));
    draw(handle, &mut visual);
    (handle, visual, connections, path)
}

fn draw(handle: WindowHandle<Latch>, cx: &mut VisualTestContext) {
    let root = handle.root(cx).unwrap();
    cx.draw(point(px(0.), px(0.)), size(px(1000.), px(800.)), |_, _| {
        root.into_any_element()
    });
}

fn open_delete(handle: WindowHandle<Latch>, cx: &mut VisualTestContext) {
    let position = cx.debug_bounds("delete-all-first").unwrap().center();
    cx.simulate_click(position, Default::default());
    draw(handle, cx);
}

#[gpui::test]
fn delete_click_requires_confirmation_and_enter_defaults_to_cancel(cx: &mut TestAppContext) {
    let (handle, mut visual, connections, path) = fixture(cx);
    let original = fs::read(&path).unwrap();
    open_delete(handle, &mut visual);
    handle
        .update(&mut visual, |app, window, _| {
            let dialog = app.connection_delete.as_ref().unwrap();
            assert_eq!(dialog.connection.id, connections[0].id);
            assert!(dialog.cancel.is_focused(window));
            assert_eq!(app.store.connections(), connections);
        })
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
    visual.simulate_keystrokes("enter");
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    handle
        .read_with(&visual, |app, _| {
            assert!(app.connection_delete.is_none());
            assert_eq!(app.store.connections(), connections);
        })
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[gpui::test]
fn escape_and_cancel_button_leave_persisted_connections_intact(cx: &mut TestAppContext) {
    let (handle, mut visual, connections, path) = fixture(cx);
    let original = fs::read(&path).unwrap();
    open_delete(handle, &mut visual);
    visual.simulate_keystrokes("escape");
    handle
        .read_with(&visual, |app, _| assert!(app.connection_delete.is_none()))
        .unwrap();
    draw(handle, &mut visual);
    open_delete(handle, &mut visual);
    let position = visual
        .debug_bounds("cancel-connection-delete")
        .unwrap()
        .center();
    visual.simulate_click(position, Default::default());
    handle
        .read_with(&visual, |app, _| {
            assert!(app.connection_delete.is_none());
            assert_eq!(app.store.connections(), connections);
        })
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[gpui::test]
fn focused_confirm_deletes_only_captured_connection_and_persists(cx: &mut TestAppContext) {
    let (handle, mut visual, connections, path) = fixture(cx);
    open_delete(handle, &mut visual);
    handle
        .update(&mut visual, |app, _, _| {
            app.store.set_sort_mode(SortMode::MostUsed).unwrap();
        })
        .unwrap();
    draw(handle, &mut visual);
    visual.simulate_keystrokes("tab");
    handle
        .update(&mut visual, |app, window, _| {
            assert!(
                app.connection_delete
                    .as_ref()
                    .unwrap()
                    .confirm
                    .is_focused(window)
            );
        })
        .unwrap();
    visual.simulate_keystrokes("enter");
    visual.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    handle
        .read_with(&visual, |app, _| {
            assert!(app.connection_delete.is_none());
            assert_eq!(app.store.connections(), &connections[1..]);
        })
        .unwrap();
    assert_eq!(Store::load(&path).unwrap().connections(), &connections[1..]);
}

#[gpui::test]
fn tab_and_shift_tab_stay_inside_confirmation(cx: &mut TestAppContext) {
    let (handle, mut visual, _, _) = fixture(cx);
    open_delete(handle, &mut visual);
    for (keys, confirm) in [
        ("tab", true),
        ("tab", false),
        ("shift-tab", true),
        ("shift-tab", false),
    ] {
        visual.simulate_keystrokes(keys);
        handle
            .update(&mut visual, |app, window, cx| {
                let dialog = app.connection_delete.as_ref().unwrap();
                assert!(dialog.focus.contains_focused(window, cx));
                assert_eq!(dialog.confirm.is_focused(window), confirm);
                assert_eq!(dialog.cancel.is_focused(window), !confirm);
            })
            .unwrap();
    }
}

#[cfg(windows)]
#[gpui::test]
fn failed_persistence_keeps_confirmation_open_and_connection_intact(cx: &mut TestAppContext) {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};

    let (handle, mut visual, connections, path) = fixture(cx);
    let original = fs::read(&path).unwrap();
    open_delete(handle, &mut visual);
    let lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let position = visual
        .debug_bounds("confirm-connection-delete")
        .unwrap()
        .center();
    visual.simulate_click(position, Default::default());
    handle
        .read_with(&visual, |app, _| {
            let dialog = app.connection_delete.as_ref().unwrap();
            assert_eq!(dialog.connection.id, connections[0].id);
            assert!(
                dialog
                    .error
                    .as_deref()
                    .unwrap()
                    .starts_with("Cannot delete connection:")
            );
            assert_eq!(app.store.connections(), connections);
        })
        .unwrap();
    drop(lock);
    assert_eq!(fs::read(&path).unwrap(), original);
}
