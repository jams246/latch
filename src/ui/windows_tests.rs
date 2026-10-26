use crate::ui::sessions::tests::{fixture, synthetic_tab};
use crate::ui::{Latch, LocalPreference, SessionKind, TabContent, files::Files};
use gpui::{AppContext, TestAppContext, WindowHandle, point, px};
use latch::ssh;

pub(in crate::ui) fn second(
    source: WindowHandle<Latch>,
    cx: &mut TestAppContext,
) -> WindowHandle<Latch> {
    let profile = source.read_with(cx, |app, _| app.profile.clone()).unwrap();
    cx.add_window(|window, cx| Latch::from_profile(profile, window.window_handle(), cx))
}

#[gpui::test]
fn moving_terminal_preserves_entity_events_usage_and_independent_closure(cx: &mut TestAppContext) {
    let (first, connection) = fixture(cx);
    let second = second(first, cx);
    let (id, terminal, events) = first
        .update(cx, |app, _, cx| {
            let (mut tab, events) = synthetic_tab(connection, SessionKind::Terminal, cx);
            tab.state = ssh::State::Connected;
            tab.counted = true;
            let TabContent::Terminal(view) = &tab.content else {
                panic!()
            };
            let terminal = view.entity_id();
            let id = tab.id.clone();
            app.tabs.push(tab);
            app.active = Some(0);
            (id, terminal, events)
        })
        .unwrap();
    first
        .update(cx, |app, window, cx| {
            assert!(app.move_tab_to(&id, second, window, cx))
        })
        .unwrap();
    second
        .update(cx, |app, window, cx| {
            assert_eq!(app.tabs[0].id, id);
            let TabContent::Terminal(view) = &app.tabs[0].content else {
                panic!()
            };
            assert_eq!(view.entity_id(), terminal);
            assert!(app.tabs[0].visible_focus(cx).is_focused(window));
            events
                .send(ssh::Event::Status(ssh::State::Connected))
                .unwrap();
            app.poll(cx);
            assert_eq!(app.store(cx).connections()[0].successful_shells, 0);
            assert!(app.move_tab_to(&id, first, window, cx));
        })
        .unwrap();
    first
        .update(cx, |app, window, cx| {
            assert!(app.move_tab_to(&id, second, window, cx))
        })
        .unwrap();
    first
        .update(cx, |app, window, cx| {
            assert!(app.request_shutdown(cx));
            window.remove_window();
        })
        .unwrap();
    second
        .read_with(cx, |app, _| assert_eq!(app.tabs[0].id, id))
        .unwrap();
}

#[gpui::test]
fn moved_sftp_routes_folder_preferences_after_source_closes(cx: &mut TestAppContext) {
    let (first, connection) = fixture(cx);
    let second = second(first, cx);
    let (id, files) = first
        .update(cx, |app, _, cx| {
            let (mut tab, _) = synthetic_tab(connection.clone(), SessionKind::Sftp, cx);
            tab.state = ssh::State::Connected;
            let (commands, _) = tokio::sync::mpsc::channel(1);
            let files = cx.new(|cx| {
                Files::new(
                    &connection,
                    commands,
                    app.runtime.clone(),
                    Some(app.directory.clone()),
                    tab.id.clone(),
                    app.preference_sender.clone(),
                    cx,
                )
            });
            if let TabContent::Sftp { files: slot, .. } = &mut tab.content {
                *slot = Some(files.clone());
            }
            let id = tab.id.clone();
            app.tabs.push(tab);
            app.active = Some(0);
            (id, files)
        })
        .unwrap();
    first
        .update(cx, |app, window, cx| {
            assert!(app.move_tab_to(&id, second, window, cx))
        })
        .unwrap();
    first
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    let path = second
        .read_with(cx, |app, _| app.directory.clone())
        .unwrap();
    files.update(cx, |files, _| {
        files
            .preference_sender
            .send(LocalPreference {
                tab_id: id,
                connection_id: connection.id.clone(),
                path: path.clone(),
            })
            .unwrap()
    });
    second
        .update(cx, |app, _, cx| app.poll_local_preferences(cx))
        .unwrap();
    crate::ui::persistence::tests::settle(second, cx);
    second
        .read_with(cx, |app, cx| {
            assert_eq!(
                app.store(cx).local_directory(&connection.id),
                Some(path.as_path())
            )
        })
        .unwrap();
}

#[gpui::test]
fn blocked_moves_and_drag_cancellation_preserve_source(cx: &mut TestAppContext) {
    let (first, connection) = fixture(cx);
    let second = second(first, cx);
    first
        .update(cx, |app, window, cx| {
            let (tab, _) = synthetic_tab(connection, SessionKind::Terminal, cx);
            let id = tab.id.clone();
            app.tabs.push(tab);
            app.active = Some(0);
            for state in [
                ssh::State::Connecting,
                ssh::State::Authenticating,
                ssh::State::Reconnecting,
            ] {
                app.tabs[0].state = state;
                assert!(!app.move_tab_to(&id, second, window, cx));
            }
            app.tabs[0].state = ssh::State::Connected;
            app.tabs[0].closing = true;
            assert!(!app.move_tab_to(&id, second, window, cx));
            app.tabs[0].closing = false;
            second
                .update(cx, |target, _, _| target.data_busy = true)
                .unwrap();
            assert!(!app.move_tab_to(&id, second, window, cx));
            app.tab_drag.origin = Some((point(px(0.), px(0.)), px(0.)));
            app.cancel_tab_drag(cx);
            assert!(app.tab_drag.origin.is_none());
            assert_eq!(app.tabs[0].id, id);
            assert!(!app.can_import(cx));
        })
        .unwrap();
}
