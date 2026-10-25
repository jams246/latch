use super::tests::{draw, fixture, key, open};
use crate::ui::{SessionKind, reconnect::ReconnectDialog, sessions::tests::synthetic_tab};
use gpui::{TestAppContext, point, px};
use latch::ssh;

#[gpui::test]
fn duplicate_is_revealed_and_ordinary_connections_still_append(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |app, window, cx| {
            let connection = app.tabs[0].connection.clone();
            for _ in 0..15 {
                let (tab, _) = synthetic_tab(connection.clone(), SessionKind::Terminal, cx);
                app.tabs.push(tab);
            }
            let source = app.tabs.len() - 1;
            app.duplicate_tab(source, window, cx);
            assert_eq!(app.active, Some(source + 1));
        })
        .unwrap();
    draw(&mut visual);
    visual.run_until_parked();
    draw(&mut visual);
    let bounds = visual.debug_bounds("tab-name-18").unwrap();
    assert!(bounds.left() >= px(310.) && bounds.right() <= px(900.));
    handle
        .update(&mut visual, |app, window, cx| {
            let connection = app.tabs[0].connection.clone();
            let last = app.tabs.last().unwrap().id.clone();
            app.connect(connection, SessionKind::Terminal, window, cx);
            assert_eq!(app.tabs[18].id, last);
            assert_eq!(app.active, Some(19));
        })
        .unwrap();
}

#[gpui::test]
fn successful_duplicate_counts_once_and_completion_does_not_steal_menu_focus(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual) = fixture(cx);
    let sender = handle
        .update(&mut visual, |app, window, cx| {
            app.duplicate_tab(0, window, cx);
            let (sender, events) = std::sync::mpsc::channel();
            app.tabs[1].events = events;
            assert!(!app.tabs[1].counted);
            assert_eq!(app.store.connections()[0].successful_shells, 0);
            sender
        })
        .unwrap();
    draw(&mut visual);
    open(&mut visual, 1);
    sender
        .send(ssh::Event::Status(ssh::State::Connected))
        .unwrap();
    sender
        .send(ssh::Event::Status(ssh::State::Connected))
        .unwrap();
    handle
        .update(&mut visual, |app, window, cx| {
            app.poll(cx);
            assert!(app.tabs[1].counted);
            assert!(app.tab_menu.as_ref().unwrap().duplicate.is_focused(window));
        })
        .unwrap();
    crate::ui::persistence::tests::settle(handle, &mut visual);
    handle
        .read_with(&visual, |app, _| {
            assert_eq!(app.store.connections()[0].successful_shells, 1);
        })
        .unwrap();
    key(&mut visual, "escape");
}

#[gpui::test]
fn closing_source_disables_duplicate_and_shutdown_clears_menu(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |app, _, _| app.tabs[0].closing = true)
        .unwrap();
    open(&mut visual, 0);
    for stroke in ["up", "down", "shift-tab", "tab"] {
        key(&mut visual, stroke);
        handle
            .update(&mut visual, |app, window, _| {
                assert!(app.tab_menu.as_ref().unwrap().close.is_focused(window));
            })
            .unwrap();
    }
    handle
        .update(&mut visual, |app, _, _| app.tabs[0].closing = false)
        .unwrap();
    key(&mut visual, "escape");
    handle
        .update(&mut visual, |app, _, cx| {
            app.tabs[0].state = ssh::State::Reconnecting;
            app.tabs[0].reconnect = Some(ReconnectDialog::new(cx));
        })
        .unwrap();
    draw(&mut visual);
    open(&mut visual, 0);
    handle
        .update(&mut visual, |app, window, _| {
            assert!(app.tab_menu.as_ref().unwrap().duplicate.is_focused(window));
        })
        .unwrap();
    key(&mut visual, "enter");
    handle
        .update(&mut visual, |app, window, cx| {
            assert_eq!(app.tabs.len(), 4);
            let id = app.tabs[1].id.clone();
            app.open_tab_menu(&id, point(px(400.), px(20.)), window, cx);
            app.request_shutdown(cx);
            assert!(app.tab_menu.is_none());
        })
        .unwrap();
}
