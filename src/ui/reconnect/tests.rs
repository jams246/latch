use crate::ui::{SessionKind, TabContent, sessions::tests::*};
use gpui::{IntoElement, TestAppContext, VisualTestContext, point, px, size};
use latch::ssh::{Event, State};

#[gpui::test]
fn recovery_preserves_terminal_and_counts_only_original_shell(cx: &mut TestAppContext) {
    let (handle, connection) = fixture(cx);
    handle
        .update(cx, |app, _, cx| {
            let (tab, events) = synthetic_tab(connection, SessionKind::Terminal, cx);
            let TabContent::Terminal(terminal) = &tab.content else {
                panic!("Expected terminal");
            };
            let terminal = terminal.clone();
            let id = tab.id.clone();
            app.tabs.push(tab);
            app.active = Some(0);
            events.send(Event::Status(State::Connected)).unwrap();
            app.poll(cx);
            events.send(Event::Status(State::Reconnecting)).unwrap();
            app.poll(cx);
            assert!(app.tabs[0].reconnect.is_some());
            assert!(!app.tabs[0].closing);
            events.send(Event::Status(State::Connected)).unwrap();
            app.poll(cx);
            assert!(app.tabs[0].reconnect.is_none());
            assert_eq!(app.tabs[0].id, id);
            assert!(
                matches!(&app.tabs[0].content, TabContent::Terminal(view) if view == &terminal)
            );
        })
        .unwrap();
    crate::ui::persistence::tests::settle(handle, cx);
    handle
        .read_with(cx, |app, _| {
            assert_eq!(app.store.connections()[0].successful_shells, 1);
        })
        .unwrap();
}

#[gpui::test]
fn dialog_shows_close_only_after_failure_and_closes_its_tab(cx: &mut TestAppContext) {
    let (handle, connection) = fixture(cx);
    let events = handle
        .update(cx, |app, _, cx| {
            let (tab, events) = synthetic_tab(connection.clone(), SessionKind::Sftp, cx);
            let (other, _) = synthetic_tab(connection, SessionKind::Terminal, cx);
            app.tabs.extend([tab, other]);
            app.active = Some(0);
            events.send(Event::Status(State::Reconnecting)).unwrap();
            app.poll(cx);
            events
        })
        .unwrap();
    let root = handle.root(cx).unwrap();
    let cx = VisualTestContext::from_window(*handle, cx).into_mut();
    let draw = |cx: &mut VisualTestContext| {
        cx.draw(point(px(0.), px(0.)), size(px(1120.), px(720.)), |_, _| {
            root.clone().into_any_element()
        });
    };
    draw(cx);
    assert!(cx.debug_bounds("reconnect-dialog").is_some());
    assert!(cx.debug_bounds("reconnect-loading").is_some());
    assert!(cx.debug_bounds("close-reconnect").is_none());
    cx.simulate_keystrokes("enter escape");
    handle
        .read_with(cx, |app, _| assert_eq!(app.tabs.len(), 2))
        .unwrap();
    events.send(Event::Status(State::Failed)).unwrap();
    handle.update(cx, |app, _, cx| app.poll(cx)).unwrap();
    draw(cx);
    let close = cx.debug_bounds("close-reconnect").unwrap().center();
    cx.simulate_click(close, Default::default());
    handle
        .read_with(cx, |app, _| {
            assert_eq!(app.tabs.len(), 1);
            assert!(matches!(app.tabs[0].content, TabContent::Terminal(_)));
            assert_eq!(app.active, Some(0));
        })
        .unwrap();
}

#[gpui::test]
fn inactive_tab_recovery_does_not_replace_active_tab(cx: &mut TestAppContext) {
    let (handle, connection) = fixture(cx);
    handle
        .update(cx, |app, window, cx| {
            let (tab, events) = synthetic_tab(connection.clone(), SessionKind::Terminal, cx);
            let (other, _) = synthetic_tab(connection, SessionKind::Sftp, cx);
            app.tabs.extend([tab, other]);
            app.active = Some(1);
            let focus = app.tabs[1].visible_focus(cx);
            window.focus(&focus);
            events.send(Event::Status(State::Reconnecting)).unwrap();
            events.send(Event::Status(State::Connected)).unwrap();
            app.poll(cx);
            assert_eq!(app.active, Some(1));
            assert!(focus.is_focused(window));
            assert!(app.tabs[0].reconnect.is_none());
        })
        .unwrap();
}
