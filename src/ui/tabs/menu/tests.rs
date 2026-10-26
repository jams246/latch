use crate::ui::{
    Latch, SessionKind,
    sessions::tests::{fixture as session_fixture, synthetic_tab},
};
use gpui::{
    KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, TestAppContext, VisualTestContext,
    WindowHandle, point, px, size,
};

pub(super) fn fixture(cx: &mut TestAppContext) -> (WindowHandle<Latch>, VisualTestContext) {
    let (handle, connection) = session_fixture(cx);
    handle
        .update(cx, |app, window, cx| {
            for kind in [
                SessionKind::Terminal,
                SessionKind::Terminal,
                SessionKind::Sftp,
            ] {
                let (tab, _) = synthetic_tab(connection.clone(), kind, cx);
                app.tabs.push(tab);
            }
            app.active = Some(0);
            window.focus(&app.tabs[0].visible_focus(cx));
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(*handle, cx);
    visual.simulate_resize(size(px(900.), px(600.)));
    draw(&mut visual);
    (handle, visual)
}

pub(super) fn draw(visual: &mut VisualTestContext) {
    visual.update(|window, cx| window.draw(cx)).clear();
}

pub(super) fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let point = visual.debug_bounds(selector).unwrap().center();
    visual.simulate_click(point, Default::default());
}

pub(super) fn open(visual: &mut VisualTestContext, index: usize) {
    let position = visual
        .debug_bounds(["tab-name-0", "tab-name-1", "tab-name-2"][index])
        .unwrap()
        .center();
    visual.simulate_mouse_down(position, MouseButton::Right, Default::default());
    visual.simulate_mouse_up(position, MouseButton::Right, Default::default());
    draw(visual);
}

pub(super) fn key(visual: &mut VisualTestContext, key: &str) {
    let keystroke = Keystroke::parse(key).unwrap();
    visual.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
    });
    visual.simulate_event(KeyUpEvent { keystroke });
    draw(visual);
}

#[gpui::test]
fn right_click_headers_and_close_buttons_keeps_active_tab_and_restores_focus(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual) = fixture(cx);
    for selector in ["tab-name-0", "tab-name-1", "tab-close-1"] {
        let point = visual.debug_bounds(selector).unwrap().center();
        visual.simulate_mouse_down(point, MouseButton::Right, Default::default());
        visual.simulate_mouse_up(point, MouseButton::Right, Default::default());
        draw(&mut visual);
        let duplicate = visual.debug_bounds("tab-menu-duplicate").unwrap();
        let close = visual.debug_bounds("tab-menu-close").unwrap();
        assert!(duplicate.bottom() <= close.top());
        handle
            .update(&mut visual, |app, window, cx| {
                assert_eq!(app.active, Some(0));
                assert_eq!(app.tabs.len(), 3);
                assert!(app.tab_drag.origin.is_none());
                assert!(!app.tab_drag.did_drag);
                assert!(app.tab_menu.as_ref().unwrap().duplicate.is_focused(window));
                assert!(!app.tabs[0].visible_focus(cx).is_focused(window));
            })
            .unwrap();
        key(&mut visual, "escape");
        handle
            .update(&mut visual, |app, window, cx| {
                assert!(app.tab_menu.is_none());
                assert!(app.tabs[0].visible_focus(cx).is_focused(window));
            })
            .unwrap();
    }
    open(&mut visual, 0);
    visual.simulate_click(point(px(400.), px(500.)), Default::default());
    draw(&mut visual);
    handle
        .update(&mut visual, |app, window, cx| {
            assert!(app.tab_menu.is_none());
            assert!(app.tabs[0].visible_focus(cx).is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn sftp_menu_only_closes_clicked_tab_and_keyboard_stays_inside_menu(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    open(&mut visual, 2);
    assert!(visual.debug_bounds("tab-menu-duplicate").is_none());
    assert!(visual.debug_bounds("tab-menu-duplicate-window").is_none());
    for stroke in ["up", "down", "tab", "shift-tab", "a"] {
        key(&mut visual, stroke);
        handle
            .update(&mut visual, |app, window, _| {
                assert!(app.tab_menu.as_ref().unwrap().close.is_focused(window));
            })
            .unwrap();
    }
    key(&mut visual, "space");
    handle
        .update(&mut visual, |app, window, cx| {
            assert_eq!(app.tabs.len(), 2);
            assert_eq!(app.active, Some(0));
            assert!(app.tabs[0].visible_focus(cx).is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn menu_snaps_to_window_edges_and_settings_clears_it(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |app, window, cx| {
            let id = app.tabs[0].id.clone();
            app.open_tab_menu(&id, point(px(899.), px(599.)), window, cx);
        })
        .unwrap();
    draw(&mut visual);
    let bounds = visual.debug_bounds("tab-context-menu").unwrap();
    assert!(bounds.left() >= px(0.) && bounds.right() <= px(900.));
    assert!(
        bounds.top() >= px(0.) && bounds.bottom() <= px(600.),
        "{bounds:?}"
    );
    handle
        .update(&mut visual, |app, window, cx| {
            app.open_settings(window, cx);
            assert!(app.tab_menu.is_none());
        })
        .unwrap();
}
