use crate::ui::{sessions::tests::fixture, windows_tests::second};
use gpui::TestAppContext;
use latch::storage::SortMode;
use std::{io, sync::mpsc, time::Duration};

#[gpui::test]
fn shared_queue_preserves_interleaved_writes_and_survives_origin_closure(cx: &mut TestAppContext) {
    let (first, connection) = fixture(cx);
    let second = second(first, cx);
    let (release, wait) = mpsc::channel();
    first
        .update(cx, |app, _, cx| {
            app.save_preference(
                "sorting",
                move |store| {
                    wait.recv_timeout(Duration::from_secs(5)).unwrap();
                    store.set_sort_mode(SortMode::MostUsed)
                },
                cx,
            );
        })
        .unwrap();
    let id = connection.id.clone();
    second
        .update(cx, |app, _, cx| {
            app.save_preference("usage", move |store| store.record_successful_shell(&id), cx);
            assert!(!app.can_import(cx));
        })
        .unwrap();
    first
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    release.send(()).unwrap();
    crate::ui::persistence::tests::settle(second, cx);
    second
        .read_with(cx, |app, cx| {
            assert_eq!(app.store(cx).sort_mode(), SortMode::MostUsed);
            assert_eq!(app.store(cx).connections()[0].successful_shells, 1);
            assert!(app.can_import(cx));
        })
        .unwrap();
}

#[gpui::test]
fn write_failure_does_not_block_another_window_or_publish_failed_state(cx: &mut TestAppContext) {
    let (first, connection) = fixture(cx);
    let second = second(first, cx);
    first
        .update(cx, |app, _, cx| {
            app.save_preference(
                "synthetic",
                |_| Err(io::Error::other("Synthetic failure")),
                cx,
            );
        })
        .unwrap();
    second
        .update(cx, |app, _, cx| {
            app.save_preference(
                "usage",
                move |store| store.record_successful_shell(&connection.id),
                cx,
            );
        })
        .unwrap();
    crate::ui::persistence::tests::settle(second, cx);
    first
        .read_with(cx, |app, cx| {
            assert!(app.error.as_ref().unwrap().contains("Synthetic failure"));
            assert_eq!(app.store(cx).connections()[0].successful_shells, 1);
            assert!(!app.saves_pending());
        })
        .unwrap();
}
