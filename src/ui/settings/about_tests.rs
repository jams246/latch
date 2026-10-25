use super::{Page, tests::fixture};
use gpui::TestAppContext;

#[gpui::test]
fn about_is_first_and_selected_each_time_settings_opens(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture.open_settings();
    let about = fixture.visual.debug_bounds("settings-about").unwrap();
    let terminal = fixture
        .visual
        .debug_bounds("settings-terminal-theme")
        .unwrap();
    let data = fixture.visual.debug_bounds("settings-data").unwrap();
    assert!(about.bottom() < terminal.top() && terminal.bottom() < data.top());
    assert!(
        fixture
            .visual
            .debug_bounds("settings-about-version")
            .is_some()
    );
    assert!(fixture.visual.debug_bounds("settings-page").is_none());
    fixture.select_terminal_theme();
    fixture
        .handle
        .read_with(&fixture.visual, |app, _| {
            assert!(app.settings.as_ref().unwrap().page == Page::Terminal);
        })
        .unwrap();
    assert!(fixture.visual.debug_bounds("settings-page").is_some());
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            app.open_settings(window, cx);
            assert!(app.settings.as_ref().unwrap().page == Page::About);
            app.close_settings(window, cx);
        })
        .unwrap();
    fixture.draw();
    fixture.open_settings();
    assert!(fixture.visual.debug_bounds("settings-about-page").is_some());
    fixture
        .visual
        .simulate_click(data.center(), Default::default());
    fixture.draw();
    assert!(fixture.visual.debug_bounds("settings-data-page").is_some());
    fixture
        .visual
        .simulate_click(about.center(), Default::default());
    fixture.draw();
    assert!(fixture.visual.debug_bounds("settings-about-page").is_some());
    fixture.visual.simulate_keystrokes("escape");
    fixture
        .handle
        .read_with(&fixture.visual, |app, _| assert!(app.settings.is_none()))
        .unwrap();
}

#[gpui::test]
fn about_navigation_respects_operation_guards_and_preserves_errors(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture.open_settings();
    fixture.select_terminal_theme();
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            app.settings.as_mut().unwrap().error = Some("Synthetic save error".into());
            app.data_busy = true;
            app.open_settings(window, cx);
            assert!(app.settings.as_ref().unwrap().page == Page::Terminal);
        })
        .unwrap();
    let about = fixture
        .visual
        .debug_bounds("settings-about")
        .unwrap()
        .center();
    fixture.visual.simulate_click(about, Default::default());
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            assert!(app.settings.as_ref().unwrap().page == Page::Terminal);
            app.data_busy = false;
            app.shutting_down = true;
            app.open_settings(window, cx);
            assert!(app.settings.as_ref().unwrap().page == Page::Terminal);
            app.shutting_down = false;
            app.open_settings(window, cx);
            assert!(app.settings.as_ref().unwrap().page == Page::About);
            assert_eq!(
                app.settings.as_ref().unwrap().error.as_deref(),
                Some("Synthetic save error")
            );
        })
        .unwrap();
    fixture.draw();
    assert!(fixture.visual.debug_bounds("settings-about-page").is_some());
}
