use super::tests::fixture;
use crate::ui::persistence::tests::settle;
use gpui::TestAppContext;
use latch::storage::{AppTheme, TerminalTheme};

#[gpui::test]
fn choosing_app_colors_preserves_the_open_terminal_and_its_preset(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture.open_settings();
    let citron = fixture
        .visual
        .debug_bounds("app-colors-Citron")
        .unwrap()
        .center();
    fixture.visual.simulate_click(citron, Default::default());
    settle(fixture.handle, &mut fixture.visual);
    fixture.draw();
    fixture.visual.simulate_keystrokes("escape");
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            assert!(app.settings.is_none());
            assert_eq!(app.store(cx).app_theme(), AppTheme::Citron);
            assert_eq!(app.store(cx).terminal_theme(), TerminalTheme::Classic);
            assert_eq!(app.tabs.len(), 1);
            assert_eq!(app.active, Some(0));
            assert!(app.tabs[0].visible_focus(cx).is_focused(window));
        })
        .unwrap();
}
