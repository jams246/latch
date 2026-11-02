use crate::ui::Latch;
use gpui::{
    KeyDownEvent, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext, WindowHandle, px, size,
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
        .join(".verification/connection-library")
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
fn settings_returns_to_the_library_with_the_same_search(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    let library = fixture.visual.debug_bounds("connection-library").unwrap();
    assert_eq!(library.size.width, px(720.));
    let tab = fixture.visual.debug_bounds("library-tab").unwrap();
    let gear = fixture.visual.debug_bounds("settings-open").unwrap();
    assert!(gear.left() > tab.right());
    fixture.click("settings-open");
    assert!(fixture.visual.debug_bounds("settings-page").is_some());
    fixture.key("escape");
    assert!(fixture.visual.debug_bounds("connection-library").is_some());
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            assert_eq!(app.connection_search.read(cx).value.as_str(), "Production");
            assert!(app.connection_search.read(cx).focus.is_focused(window));
        })
        .unwrap();
}
