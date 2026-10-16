use crate::ui::Latch;
use gpui::{KeyUpEvent, Keystroke, TestAppContext, VisualTestContext};
use latch::storage::Store;
use std::sync::Arc;

#[gpui::test]
fn editor_should_accept_typing_and_save_after_keyboard_navigation(cx: &mut TestAppContext) {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/app-updates/editor")
        .join(uuid::Uuid::new_v4().to_string());
    let store = Store::load(directory.join("latch.json")).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    handle
        .update(cx, |app, window, cx| app.edit(None, window, cx))
        .unwrap();
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
