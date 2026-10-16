use crate::ui::Latch;
use gpui::{
    KeyDownEvent, KeyUpEvent, Keystroke, MouseButton, TestAppContext, VisualTestContext,
    WindowHandle, point, px, size,
};
use latch::storage::Store;
use std::{path::PathBuf, sync::Arc};

struct Fixture {
    handle: WindowHandle<Latch>,
    visual: VisualTestContext,
}

impl Fixture {
    fn draw(&mut self) {
        self.visual.update(|window, cx| window.draw(cx)).clear();
    }

    fn click(&mut self, selector: &'static str) {
        let position = self.visual.debug_bounds(selector).unwrap().center();
        self.visual.simulate_click(position, Default::default());
        self.visual.run_until_parked();
        self.draw();
    }

    fn key(&mut self, key: &str) {
        let keystroke = Keystroke::parse(key).unwrap();
        self.visual.simulate_event(KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
        });
        self.visual.simulate_event(KeyUpEvent { keystroke });
        self.visual.run_until_parked();
        self.draw();
    }
}

fn fixture(cx: &mut TestAppContext) -> Fixture {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/sidebar-toggle")
        .join(uuid::Uuid::new_v4().to_string());
    let store = Store::load(directory.join("latch.db")).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    handle
        .update(cx, |app, _, cx| {
            app.connection_search
                .update(cx, |input, cx| input.replace(0..0, "Production", cx));
        })
        .unwrap();
    let visual = VisualTestContext::from_window(*handle, cx);
    visual.simulate_resize(size(px(720.), px(480.)));
    visual.run_until_parked();
    let mut fixture = Fixture { handle, visual };
    fixture.draw();
    fixture
}

#[gpui::test]
fn sidebar_toggle_resizes_workspace_preserves_search_and_keeps_settings_usable(
    cx: &mut TestAppContext,
) {
    let mut fixture = fixture(cx);
    let sidebar = fixture.visual.debug_bounds("sidebar").unwrap();
    let workspace = fixture.visual.debug_bounds("workspace").unwrap();
    let settings = fixture.visual.debug_bounds("settings-open").unwrap();
    let toggle = fixture.visual.debug_bounds("sidebar-toggle").unwrap();
    let search = fixture
        .handle
        .read_with(&fixture.visual, |app, _| {
            assert!(!app.sidebar_collapsed);
            app.connection_search.entity_id()
        })
        .unwrap();
    assert_eq!(sidebar.size.width, px(310.));
    assert!(toggle.left() >= settings.right());
    assert!(fixture.visual.debug_bounds("connections-sidebar").is_some());

    fixture.click("sidebar-toggle");
    fixture
        .handle
        .read_with(&fixture.visual, |app, _| assert!(app.sidebar_collapsed))
        .unwrap();
    assert!(fixture.visual.debug_bounds("sidebar-toggle").is_some());
    assert_eq!(
        fixture.visual.debug_bounds("sidebar").unwrap().size.width,
        px(60.)
    );
    assert_eq!(
        fixture.visual.debug_bounds("workspace").unwrap().size.width,
        workspace.size.width + px(250.)
    );
    fixture
        .handle
        .read_with(&fixture.visual, |app, cx| {
            assert!(app.sidebar_collapsed);
            assert_eq!(app.connection_search.entity_id(), search);
            assert_eq!(app.connection_search.read(cx).value.as_str(), "Production");
        })
        .unwrap();

    fixture.click("sidebar-toggle");
    assert!(fixture.visual.debug_bounds("connections-sidebar").is_some());
    assert_eq!(fixture.visual.debug_bounds("sidebar").unwrap(), sidebar);
    assert_eq!(fixture.visual.debug_bounds("workspace").unwrap(), workspace);
    fixture.click("settings-open");
    assert!(fixture.visual.debug_bounds("settings-page").is_some());
    fixture.click("settings-back");
    assert!(fixture.visual.debug_bounds("connections-sidebar").is_some());
    fixture
        .handle
        .read_with(&fixture.visual, |app, cx| {
            assert!(app.settings.is_none());
            assert_eq!(app.connection_search.read(cx).value.as_str(), "Production");
        })
        .unwrap();
}

#[gpui::test]
fn sidebar_toggle_keeps_keyboard_focus_and_settings_navigation_expanded(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    let position = fixture
        .visual
        .debug_bounds("sidebar-toggle")
        .unwrap()
        .center();
    fixture
        .visual
        .simulate_mouse_down(position, MouseButton::Left, Default::default());
    fixture
        .visual
        .simulate_mouse_up(point(px(0.), px(0.)), MouseButton::Left, Default::default());
    fixture.draw();

    fixture.key("enter");
    fixture
        .handle
        .read_with(&fixture.visual, |app, _| assert!(app.sidebar_collapsed))
        .unwrap();
    assert_eq!(
        fixture.visual.debug_bounds("sidebar").unwrap().size.width,
        px(60.)
    );
    fixture.key("space");
    assert!(fixture.visual.debug_bounds("connections-sidebar").is_some());
    fixture.key("enter");
    assert_eq!(
        fixture.visual.debug_bounds("sidebar").unwrap().size.width,
        px(60.)
    );

    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            app.open_settings(window, cx);
        })
        .unwrap();
    fixture.draw();
    assert_eq!(
        fixture.visual.debug_bounds("sidebar").unwrap().size.width,
        px(310.)
    );
    assert!(fixture.visual.debug_bounds("settings-back").is_some());
    fixture.click("settings-back");
    assert_eq!(
        fixture.visual.debug_bounds("sidebar").unwrap().size.width,
        px(60.)
    );
    assert!(fixture.visual.debug_bounds("sidebar-toggle").is_some());
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, _| {
            assert!(app.sidebar_toggle_focus.is_focused(window));
        })
        .unwrap();
    fixture.key("enter");
    assert!(fixture.visual.debug_bounds("connections-sidebar").is_some());
}
