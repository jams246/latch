use super::tests::{click, draw, fixture, key, open};
use crate::ui::{SessionKind, TabContent, sessions::tests::synthetic_tab};
use gpui::TestAppContext;
use latch::ssh;

#[gpui::test]
fn duplicate_uses_source_snapshot_and_inserts_independent_terminal_next_to_source(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual) = fixture(cx);
    let (source, other, terminal) = handle
        .update(&mut visual, |app, _, cx| {
            app.tabs[1].connection.name = "Unsaved snapshot".into();
            app.update_store(cx, |store| store.delete(&app.tabs[1].connection.id.clone()))
                .unwrap();
            let TabContent::Terminal(terminal) = &app.tabs[1].content else {
                panic!()
            };
            (
                app.tabs[1].id.clone(),
                app.tabs[2].id.clone(),
                terminal.entity_id(),
            )
        })
        .unwrap();
    open(&mut visual, 1);
    click(&mut visual, "tab-menu-duplicate");
    draw(&mut visual);
    handle
        .update(&mut visual, |app, window, cx| {
            assert_eq!(app.tabs.len(), 4);
            assert_eq!(app.active, Some(2));
            assert_eq!(app.tabs[1].id, source);
            assert_eq!(app.tabs[3].id, other);
            let copy = &app.tabs[2];
            assert_ne!(copy.id, source);
            assert_eq!(copy.name, "Unsaved snapshot");
            assert_eq!(copy.connection.id, app.tabs[1].connection.id);
            assert!(!copy.counted);
            assert!(
                copy.password_attempt.is_none()
                    && copy.dialog.is_none()
                    && copy.host_dialog.is_none()
            );
            let TabContent::Terminal(view) = &copy.content else {
                panic!()
            };
            assert_ne!(view.entity_id(), terminal);
            assert!(copy.visible_focus(cx).is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn duplicate_guards_and_source_states_use_existing_connection_flow(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |app, window, cx| {
            for state in [
                ssh::State::Connecting,
                ssh::State::Authenticating,
                ssh::State::Connected,
                ssh::State::Failed,
                ssh::State::Disconnected,
                ssh::State::Reconnecting,
            ] {
                app.tabs[0].state = state;
                app.duplicate_tab(0, window, cx);
                assert_eq!(app.tabs.len(), 4);
                app.close_tab(1, window, cx);
            }
            app.tabs[0].closing = true;
            app.duplicate_tab(0, window, cx);
            assert_eq!(app.tabs.len(), 3);
            app.tabs[0].closing = false;
            app.data_busy = true;
            app.duplicate_tab(0, window, cx);
            assert_eq!(app.tabs.len(), 3);
            app.data_busy = false;
            app.shutting_down = true;
            app.duplicate_tab(0, window, cx);
            assert_eq!(app.tabs.len(), 3);
            app.shutting_down = false;
            app.duplicate_tab(2, window, cx);
            app.duplicate_tab(99, window, cx);
            assert_eq!(app.tabs.len(), 3);
        })
        .unwrap();
}

#[gpui::test]
fn close_resolves_stable_id_after_index_shift_and_last_tab_restores_sidebar_focus(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual) = fixture(cx);
    open(&mut visual, 1);
    handle
        .update(&mut visual, |app, window, cx| app.close_tab(0, window, cx))
        .unwrap();
    draw(&mut visual);
    key(&mut visual, "down");
    key(&mut visual, "down");
    key(&mut visual, "enter");
    handle
        .update(&mut visual, |app, window, cx| {
            assert_eq!(app.tabs.len(), 1);
            assert!(matches!(app.tabs[0].content, TabContent::Sftp { .. }));
            assert_eq!(app.active, Some(0));
            app.close_tab(0, window, cx);
            assert!(app.connection_search.read(cx).focus.is_focused(window));
            let (tab, _) = synthetic_tab(
                app.store(cx).connections()[0].clone(),
                SessionKind::Terminal,
                cx,
            );
            app.tabs.push(tab);
            app.active = Some(0);
            app.sidebar_collapsed = true;
            app.close_tab(0, window, cx);
            assert!(app.sidebar_toggle_focus.is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn removed_target_dismisses_without_acting_on_another_tab(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    open(&mut visual, 1);
    handle
        .update(&mut visual, |app, window, cx| {
            app.tabs.remove(1);
            app.choose_tab_action(super::Action::Close, window, cx);
            assert_eq!(app.tabs.len(), 2);
            assert!(app.tab_menu.is_none());
        })
        .unwrap();
    draw(&mut visual);
    open(&mut visual, 0);
    handle
        .update(&mut visual, |app, window, cx| {
            app.close_tab(0, window, cx);
            assert!(app.tab_menu.is_none());
        })
        .unwrap();
}
