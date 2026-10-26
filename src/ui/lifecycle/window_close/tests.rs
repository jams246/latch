use crate::ui::{Latch, sessions::tests::fixture, windows_tests::second};
use gpui::{TestAppContext, WindowHandle};
use latch::storage::WindowSize;
use std::time::{Duration, Instant};

fn closed(handle: WindowHandle<Latch>, cx: &mut TestAppContext) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if handle.read_with(cx, |_, _| ()).is_err() {
            break;
        }
        assert!(Instant::now() < deadline, "Window did not close");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[gpui::test]
fn last_closed_window_supplies_dimensions_after_its_cleanup(cx: &mut TestAppContext) {
    let (first, _) = fixture(cx);
    let second = second(first, cx);
    let profile = first.read_with(cx, |app, _| app.profile.clone()).unwrap();
    first
        .update(cx, |app, _, cx| {
            app.closing_size = Some(WindowSize {
                width: 900.,
                height: 600.,
                maximized: false,
            });
            app.pending_cleanup = 1;
            assert!(!app.request_shutdown(cx));
            app.poll_cleanup(cx);
            assert!(!app.saves_pending());
        })
        .unwrap();
    second
        .update(cx, |app, _, cx| {
            app.closing_size = Some(WindowSize::default());
            assert!(app.request_shutdown(cx));
            app.finish_window_close(cx);
        })
        .unwrap();
    closed(second, cx);
    first
        .update(cx, |app, _, cx| {
            app.pending_cleanup = 0;
            app.poll_cleanup(cx);
        })
        .unwrap();
    closed(first, cx);
    cx.update(|cx| {
        assert_eq!(profile.read(cx).store.window_size().width, 900.);
        assert_eq!(profile.read(cx).store.window_size().height, 600.);
    });
}
