use crate::ui::Latch;
use gpui::{TestAppContext, WindowHandle};
use latch::storage::{AuthMethod, Connection, SortMode, Store, TerminalTheme};
use std::{
    io,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

pub(in crate::ui) fn settle(handle: WindowHandle<Latch>, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cx.run_until_parked();
        if !handle.read_with(cx, |app, _| app.saves_pending()).unwrap() {
            return;
        }
        assert!(Instant::now() < deadline, "Background save did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn fixture(cx: &mut TestAppContext) -> (WindowHandle<Latch>, Connection, PathBuf) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/background-preferences")
        .join(uuid::Uuid::new_v4().to_string());
    let path = directory.join("latch.db");
    let mut store = Store::load(&path).unwrap();
    let connection = Connection::new(
        "Synthetic".into(),
        "example.invalid".into(),
        "tester".into(),
        AuthMethod::Password,
    );
    store.upsert(connection.clone()).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    (handle, connection, path)
}

#[gpui::test]
fn saves_leave_ui_available_and_serialize_preferences_edits_and_usage(cx: &mut TestAppContext) {
    let (handle, mut connection, path) = fixture(cx);
    let id = connection.id.clone();
    let (release, wait) = mpsc::channel();
    let ui_thread = std::thread::current().id();
    handle
        .update(cx, |app, _, cx| {
            app.save_preference(
                "sorting",
                move |store| {
                    assert_ne!(std::thread::current().id(), ui_thread);
                    wait.recv_timeout(Duration::from_secs(5)).unwrap();
                    store.set_sort_mode(SortMode::MostUsed)
                },
                cx,
            );
            let id = id.clone();
            app.save_preference("usage", move |store| store.record_successful_shell(&id), cx);
            connection.name = "Edited".into();
            app.save_preference("connection", move |store| store.upsert(connection), cx);
            app.save_preference(
                "theme",
                |store| store.set_terminal_theme(TerminalTheme::Carbon),
                cx,
            );
            assert_eq!(app.store(cx).sort_mode(), SortMode::Name);
            assert!(app.saves_pending());
            app.connection_search
                .update(cx, |input, cx| input.replace(0..0, "Responsive", cx));
        })
        .unwrap();
    release.send(()).unwrap();
    settle(handle, cx);
    let saved = Store::load(path).unwrap();
    assert_eq!(saved.sort_mode(), SortMode::MostUsed);
    assert_eq!(saved.terminal_theme(), TerminalTheme::Carbon);
    assert_eq!(saved.connections()[0].name, "Edited");
    assert_eq!(saved.connections()[0].successful_shells, 1);
}

#[gpui::test]
fn shutdown_waits_for_save_and_failure_keeps_window_open_with_previous_state(
    cx: &mut TestAppContext,
) {
    let (handle, _, _) = fixture(cx);
    let (release, wait) = mpsc::channel();
    handle
        .update(cx, |app, _, cx| {
            app.save_preference(
                "sorting",
                move |_| {
                    wait.recv_timeout(Duration::from_secs(5)).unwrap();
                    Err(io::Error::other("Synthetic persistence failure"))
                },
                cx,
            );
            assert!(!app.request_shutdown(cx));
            assert!(app.shutting_down);
            app.poll_cleanup(cx);
            assert!(app.saves_pending());
        })
        .unwrap();
    release.send(()).unwrap();
    settle(handle, cx);
    handle
        .read_with(cx, |app, cx| {
            assert!(!app.shutting_down);
            assert_eq!(app.store(cx).sort_mode(), SortMode::Name);
            assert!(
                app.error
                    .as_ref()
                    .unwrap()
                    .contains("Synthetic persistence failure")
            );
        })
        .unwrap();
}

#[gpui::test]
fn failed_window_save_is_visible_in_settings_and_second_close_can_exit(cx: &mut TestAppContext) {
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};
    let (handle, _, path) = fixture(cx);
    let _lock = OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path)
        .unwrap();
    handle
        .update(cx, |app, window, cx| {
            app.open_settings(window, cx);
            app.closing_size = Some(app.store(cx).window_size());
            app.finish_window_close(cx);
            assert!(!app.request_shutdown(cx));
        })
        .unwrap();
    settle(handle, cx);
    handle
        .update(cx, |app, _, cx| {
            assert!(!app.shutting_down);
            assert!(
                app.settings
                    .as_ref()
                    .unwrap()
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("Close again")
            );

            assert!(!app.saves_pending());
            assert!(app.request_shutdown(cx));
        })
        .unwrap();
}
