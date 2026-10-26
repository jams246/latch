use super::tests::{click, draw, fixture, open};
use crate::ui::{Latch, TabContent};
use gpui::TestAppContext;

#[gpui::test]
fn duplicate_window_menu_creates_fresh_terminal_and_keeps_source(cx: &mut TestAppContext) {
    let (source, mut visual) = fixture(cx);
    source
        .update(&mut visual, |app, _, _| {
            app.tabs[1].connection.name = "Snapshot".into()
        })
        .unwrap();
    open(&mut visual, 1);
    let same = visual.debug_bounds("tab-menu-duplicate").unwrap();
    let separate = visual.debug_bounds("tab-menu-duplicate-window").unwrap();
    let close = visual.debug_bounds("tab-menu-close").unwrap();
    assert!(same.bottom() <= separate.top() && separate.bottom() <= close.top());
    click(&mut visual, "tab-menu-duplicate-window");
    draw(&mut visual);
    let target = visual
        .windows()
        .into_iter()
        .filter_map(|handle| handle.downcast::<Latch>())
        .find(|handle| handle.window_id() != source.window_id())
        .unwrap();
    let (profile, original) = source
        .read_with(&visual, |app, _| {
            assert_eq!(app.tabs.len(), 3);
            (app.profile.clone(), app.tabs[1].id.clone())
        })
        .unwrap();
    target
        .update(&mut visual, |app, window, cx| {
            assert_eq!(app.profile, profile);
            assert_eq!(app.tabs.len(), 1);
            assert_eq!(app.active, Some(0));
            assert_eq!(app.tabs[0].name, "Snapshot");
            assert_ne!(app.tabs[0].id, original);
            assert!(!app.tabs[0].counted);
            assert!(matches!(app.tabs[0].content, TabContent::Terminal(_)));
            assert!(app.tabs[0].visible_focus(cx).is_focused(window));
        })
        .unwrap();
    source
        .update(&mut visual, |_, window, _| window.activate_window())
        .unwrap();
    draw(&mut visual);
    super::tests::open(&mut visual, 2);
    source
        .read_with(&visual, |app, _| {
            assert_eq!(app.tab_menu.as_ref().unwrap().tab_id, app.tabs[2].id);
        })
        .unwrap();
    assert!(visual.debug_bounds("tab-context-menu").unwrap().size.height < gpui::px(60.));
}

#[gpui::test]
fn keyboard_visits_all_menu_actions_in_both_directions(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    open(&mut visual, 0);
    for stroke in ["down", "up", "shift-tab", "tab", "tab"] {
        super::tests::key(&mut visual, stroke);
    }
    handle
        .update(&mut visual, |app, window, _| {
            assert!(
                app.tab_menu
                    .as_ref()
                    .unwrap()
                    .duplicate_window
                    .is_focused(window)
            );
        })
        .unwrap();
}
