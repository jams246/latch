use crate::ui::{Latch, Tab, TabContent};
use gpui::{
    AppContext, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, Pixels, Point,
    ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, WindowHandle, point, px,
    size,
};
use latch::{
    ssh,
    storage::{AuthMethod, Connection, Store},
    terminal::{TerminalState, TerminalView},
};
use std::sync::Arc;

const TAB_COUNT: usize = 20;

fn fixture(cx: &mut TestAppContext) -> (WindowHandle<Latch>, VisualTestContext) {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/tab-scrolling")
        .join(uuid::Uuid::new_v4().to_string());
    let store = Store::load(directory.join("latch.db")).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    handle
        .update(cx, |app, _, cx| {
            for index in 0..TAB_COUNT {
                let connection = Connection::new(
                    format!("Connection {index:02}"),
                    "example.invalid".into(),
                    "tester".into(),
                    AuthMethod::Password,
                );
                let terminal = cx
                    .new(|cx| TerminalView::new(TerminalState::new(80, 24), Arc::new(|_| {}), cx));
                let (_, events) = std::sync::mpsc::channel();
                let (_, prompts) = tokio::sync::mpsc::channel(1);
                app.tabs.push(Tab {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: connection.name.clone(),
                    destination: "tester@example.invalid:22".into(),
                    connection,
                    state: ssh::State::Connected,
                    error: None,
                    counted: true,
                    save_password: true,
                    password_attempt: None,
                    transport: None,
                    content: TabContent::Terminal(terminal),
                    dialog: None,
                    host_dialog: None,
                    reconnect: None,
                    events,
                    prompts,
                    closing: false,
                });
            }
            app.active = Some(0);
            cx.notify();
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(*handle, cx);
    visual.simulate_resize(size(px(720.), px(480.)));
    draw(handle, &mut visual);
    (handle, visual)
}

fn draw(handle: WindowHandle<Latch>, cx: &mut VisualTestContext) {
    let root = handle.root(cx).unwrap();
    cx.draw(point(px(0.), px(0.)), size(px(720.), px(480.)), |_, _| {
        root.into_any_element()
    });
}

fn wheel(cx: &mut VisualTestContext, lines: f32) {
    let position = cx.debug_bounds("tabs").unwrap().center();
    cx.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Lines(point(0., lines)),
        ..Default::default()
    });
}

fn offset(handle: WindowHandle<Latch>, cx: &VisualTestContext) -> Pixels {
    handle
        .read_with(cx, |app, _| app.tab_scroll.offset().x)
        .unwrap()
}

fn drag(cx: &mut VisualTestContext, start: Point<Pixels>, end: Point<Pixels>) {
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Default::default());
}

#[gpui::test]
fn vertical_wheel_reaches_first_and_last_tabs_at_minimum_window_size(cx: &mut TestAppContext) {
    let (handle, mut cx) = fixture(cx);
    assert_eq!(offset(handle, &cx), px(0.));
    wheel(&mut cx, -3.);
    let scrolled = offset(handle, &cx);
    assert!(scrolled < px(0.));
    wheel(&mut cx, 1.);
    assert!(offset(handle, &cx) > scrolled);

    wheel(&mut cx, -1000.);
    draw(handle, &mut cx);
    handle
        .read_with(&cx, |app, _| {
            assert_eq!(
                app.tab_scroll.offset().x,
                -app.tab_scroll.max_offset().width
            );
            assert_eq!(app.active, Some(0));
        })
        .unwrap();
    let viewport = cx.debug_bounds("tabs").unwrap();
    let last = cx.debug_bounds("tab-name-19").unwrap();
    assert!(last.left() >= viewport.left() && last.right() <= viewport.right());

    wheel(&mut cx, 1000.);
    draw(handle, &mut cx);
    assert_eq!(offset(handle, &cx), px(0.));
    let first = cx.debug_bounds("tab-name-0").unwrap();
    assert!(first.left() >= viewport.left() && first.right() <= viewport.right());
}

#[gpui::test]
fn selecting_active_tab_reveals_it_after_manual_scrolling(cx: &mut TestAppContext) {
    let (handle, mut cx) = fixture(cx);
    handle
        .update(&mut cx, |app, _, cx| {
            app.active = Some(TAB_COUNT - 1);
            cx.notify();
        })
        .unwrap();
    draw(handle, &mut cx);
    assert!(offset(handle, &cx) < px(0.));
    let viewport = cx.debug_bounds("tabs").unwrap();
    let last = cx.debug_bounds("tab-name-19").unwrap();
    assert!(last.left() >= viewport.left() && last.right() <= viewport.right());

    wheel(&mut cx, 1000.);
    draw(handle, &mut cx);
    assert_eq!(offset(handle, &cx), px(0.));
    handle
        .update(&mut cx, |app, _, cx| {
            app.active = Some(0);
            cx.notify();
        })
        .unwrap();
    draw(handle, &mut cx);
    assert_eq!(offset(handle, &cx), px(0.));
}

#[gpui::test]
fn dragging_from_close_icon_back_to_it_never_closes_tab(cx: &mut TestAppContext) {
    let (handle, mut cx) = fixture(cx);
    let start = cx.debug_bounds("tab-close-0").unwrap().center();
    drag(&mut cx, start, start - point(px(100.), px(0.)));
    assert!(offset(handle, &cx) < px(0.));
    cx.simulate_mouse_move(start, Some(MouseButton::Left), Default::default());
    cx.simulate_mouse_up(start, MouseButton::Left, Default::default());
    handle
        .read_with(&cx, |app, _| {
            assert_eq!(app.tabs.len(), TAB_COUNT);
            assert_eq!(app.active, Some(0));
        })
        .unwrap();

    draw(handle, &mut cx);
    let close = cx.debug_bounds("tab-close-0").unwrap().center();
    cx.simulate_click(close, Default::default());
    handle
        .read_with(&cx, |app, _| assert_eq!(app.tabs.len(), TAB_COUNT - 1))
        .unwrap();
}

#[gpui::test]
fn dragging_name_does_not_select_and_release_outside_clears_gesture(cx: &mut TestAppContext) {
    let (handle, mut cx) = fixture(cx);
    let start = cx.debug_bounds("tab-name-1").unwrap().center();
    let end = start - point(px(100.), px(0.));
    drag(&mut cx, start, end);
    let outside = point(end.x, px(200.));
    cx.simulate_mouse_move(outside, Some(MouseButton::Left), Default::default());
    cx.simulate_mouse_up(outside, MouseButton::Left, Default::default());
    handle
        .read_with(&cx, |app, _| {
            assert_eq!(app.tabs.len(), TAB_COUNT);
            assert_eq!(app.active, Some(0));
            assert!(app.tab_drag.origin.is_none());
        })
        .unwrap();

    wheel(&mut cx, 1000.);
    draw(handle, &mut cx);
    let name = cx.debug_bounds("tab-name-1").unwrap().center();
    cx.simulate_click(name, Default::default());
    handle
        .read_with(&cx, |app, _| assert_eq!(app.active, Some(1)))
        .unwrap();
}

#[gpui::test]
fn keyboard_can_close_tab_after_suppressed_mouse_drag(cx: &mut TestAppContext) {
    for key in ["enter", "space"] {
        let (handle, mut visual) = fixture(cx);
        visual.update(|window, cx| window.draw(cx)).clear();
        let start = visual.debug_bounds("tab-close-0").unwrap().center();
        drag(&mut visual, start, start - point(px(100.), px(0.)));
        visual.simulate_mouse_move(start, Some(MouseButton::Left), Default::default());
        visual.simulate_mouse_up(start, MouseButton::Left, Default::default());
        visual.update(|window, cx| window.draw(cx)).clear();
        handle
            .read_with(&visual, |app, _| {
                assert_eq!(app.tabs.len(), TAB_COUNT);
                assert!(app.tab_drag.did_drag);
            })
            .unwrap();

        let keystroke = Keystroke::parse(key).unwrap();
        visual.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
        });
        visual.simulate_event(KeyUpEvent { keystroke });
        handle
            .read_with(&visual, |app, _| {
                assert_eq!(app.tabs.len(), TAB_COUNT - 1, "{key} should close the tab");
            })
            .unwrap();
    }
}
