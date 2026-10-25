use super::tests::fixture;
use crate::ui::TabContent;
use gpui::{
    Focusable, KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, TestAppContext, point, px,
};
use latch::{ssh, terminal::TerminalPalette};

#[gpui::test]
fn connecting_session_cannot_steal_settings_focus_and_close_without_active_tab_focuses_search(
    cx: &mut TestAppContext,
) {
    let mut fixture = fixture(cx);
    let (events, receiver) = std::sync::mpsc::channel();
    fixture
        .handle
        .update(&mut fixture.visual, |app, _, cx| {
            app.tabs[0].state = ssh::State::Connecting;
            app.tabs[0].events = receiver;
            cx.notify();
        })
        .unwrap();
    fixture.open_settings();
    events
        .send(ssh::Event::Status(ssh::State::Connected))
        .unwrap();
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            app.poll(cx);
            assert!(app.settings.as_ref().unwrap().focus.is_focused(window));
            assert_eq!(app.tabs[0].state, ssh::State::Connected);
            let TabContent::Terminal(terminal) = &app.tabs[0].content else {
                panic!("Connection completion replaced the terminal");
            };
            assert_eq!(terminal.entity_id(), fixture.terminal);
            assert!(!terminal.focus_handle(cx).is_focused(window));
            app.active = None;
            app.close_settings(window, cx);
            assert!(app.settings.is_none());
            assert!(app.connection_search.read(cx).focus.is_focused(window));
        })
        .unwrap();
}

fn release_key(fixture: &mut super::tests::Fixture, key: &str) {
    let keystroke = Keystroke::parse(key).unwrap();
    fixture.visual.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
    });
    fixture.visual.simulate_event(KeyUpEvent { keystroke });
    fixture.draw();
}

#[gpui::test]
fn settings_buttons_support_native_keyboard_activation_and_preview_escape(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    focus_button(&mut fixture, "settings-open");
    release_key(&mut fixture, "enter");
    fixture
        .handle
        .read_with(&fixture.visual, |app, _| assert!(app.settings.is_some()))
        .unwrap();
    focus_button(&mut fixture, "settings-terminal-theme");
    release_key(&mut fixture, "enter");
    focus_button(&mut fixture, "settings-palette-Carbon");
    release_key(&mut fixture, "space");
    crate::ui::persistence::tests::settle(fixture.handle, &mut fixture.visual);
    fixture
        .handle
        .read_with(&fixture.visual, |app, cx| {
            assert_eq!(TerminalPalette::current(cx), TerminalPalette::Carbon);
            assert_eq!(app.store.terminal_theme(), TerminalPalette::Carbon);
        })
        .unwrap();
    focus_button(&mut fixture, "settings-terminal-theme");
    release_key(&mut fixture, "enter");
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, _| {
            assert!(app.settings.as_ref().unwrap().focus.is_focused(window));
        })
        .unwrap();
    focus_button(&mut fixture, "settings-back");
    release_key(&mut fixture, "space");
    fixture
        .handle
        .read_with(&fixture.visual, |app, _| assert!(app.settings.is_none()))
        .unwrap();
    fixture.open_settings();
    fixture.select_terminal_theme();
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            let preview = &app.settings.as_ref().unwrap().preview;
            window.focus(&preview.focus_handle(cx));
        })
        .unwrap();
    fixture.draw();
    release_key(&mut fixture, "tab");
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            let preview = &app.settings.as_ref().unwrap().preview;
            assert!(!preview.focus_handle(cx).is_focused(window));
            window.focus(&preview.focus_handle(cx));
        })
        .unwrap();
    fixture.draw();
    release_key(&mut fixture, "escape");
    fixture
        .handle
        .read_with(&fixture.visual, |app, _| assert!(app.settings.is_none()))
        .unwrap();
}

fn focus_button(fixture: &mut super::tests::Fixture, selector: &'static str) {
    let position = fixture.visual.debug_bounds(selector).unwrap().center();
    fixture
        .visual
        .simulate_mouse_down(position, MouseButton::Left, Default::default());
    fixture
        .visual
        .simulate_mouse_up(point(px(0.), px(0.)), MouseButton::Left, Default::default());
    fixture.draw();
}
