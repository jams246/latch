use crate::ui::{Latch, Tab, TabContent};
use gpui::{
    AppContext, EntityId, Focusable, IntoElement, ScrollDelta, ScrollWheelEvent, TestAppContext,
    VisualTestContext, WindowHandle, point, px, size,
};
use latch::{
    ssh,
    storage::{AuthMethod, Connection, Store},
    terminal::{TerminalInput, TerminalPalette, TerminalState, TerminalView},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const SELECTORS: [&str; 5] = [
    "settings-palette-Classic",
    "settings-palette-Carbon",
    "settings-palette-Glacier",
    "settings-palette-Ember",
    "settings-palette-Midnight",
];

pub(super) struct Fixture {
    pub(super) handle: WindowHandle<Latch>,
    pub(super) visual: VisualTestContext,
    path: PathBuf,
    pub(super) terminal: EntityId,
    data: Arc<AtomicUsize>,
}

impl Fixture {
    pub(super) fn draw(&mut self) {
        let root = self.handle.root(&mut self.visual).unwrap();
        self.visual
            .draw(point(px(0.), px(0.)), size(px(720.), px(480.)), |_, _| {
                root.into_any_element()
            });
    }

    pub(super) fn open_settings(&mut self) {
        let position = self.visual.debug_bounds("settings-open").unwrap().center();
        self.visual.simulate_click(position, Default::default());
        self.visual.run_until_parked();
        self.draw();
    }
}

pub(super) fn fixture(cx: &mut TestAppContext) -> Fixture {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/terminal-settings")
        .join(uuid::Uuid::new_v4().to_string());
    let path = directory.join("latch.db");
    let store = Store::load(&path).unwrap();
    store.save().unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    cx.update(|cx| cx.set_global(TerminalPalette::Classic));
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    let data = Arc::new(AtomicUsize::new(0));
    let terminal = handle
        .update(cx, |app, _, cx| {
            app.connection_search.update(cx, |input, cx| {
                input.replace(0..0, "Production", cx);
            });
            let data = data.clone();
            let terminal = cx.new(|cx| {
                TerminalView::new(
                    TerminalState::new(80, 24),
                    Arc::new(move |input| {
                        if matches!(input, TerminalInput::Data(_)) {
                            data.fetch_add(1, Ordering::Relaxed);
                        }
                    }),
                    cx,
                )
            });
            let terminal_id = terminal.entity_id();
            let connection = Connection::new(
                "Synthetic server".into(),
                "example.invalid".into(),
                "tester".into(),
                AuthMethod::Password,
            );
            let (_, events) = std::sync::mpsc::channel();
            let (_, prompts) = tokio::sync::mpsc::channel(1);
            app.tabs.push(Tab {
                id: "settings-test-terminal".into(),
                name: connection.name.clone(),
                destination: "tester@example.invalid:22".into(),
                connection,
                state: ssh::State::Connected,
                error: None,
                counted: true,
                transport: None,
                content: TabContent::Terminal(terminal),
                dialog: None,
                host_dialog: None,
                reconnect: None,
                events,
                prompts,
                closing: false,
            });
            app.active = Some(0);
            cx.notify();
            terminal_id
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(*handle, cx);
    visual.simulate_resize(size(px(720.), px(480.)));
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx)).clear();
    Fixture {
        handle,
        visual,
        path,
        terminal,
        data,
    }
}

#[gpui::test]
fn settings_save_every_palette_with_live_preview_and_preserve_hidden_session(
    cx: &mut TestAppContext,
) {
    let mut fixture = fixture(cx);
    assert!(fixture.visual.debug_bounds("settings-page").is_none());
    fixture.open_settings();
    assert!(
        fixture
            .visual
            .debug_bounds("settings-terminal-theme")
            .is_some()
    );
    assert!(fixture.visual.debug_bounds("settings-page").is_some());
    fixture.visual.simulate_keystrokes("a b c");

    for (palette, selector) in TerminalPalette::ALL.into_iter().zip(SELECTORS) {
        let bounds = fixture.visual.debug_bounds(selector).unwrap();
        assert!(
            bounds.left() >= px(310.) && bounds.right() <= px(720.) && bounds.bottom() <= px(480.)
        );
        fixture
            .visual
            .simulate_click(bounds.center(), Default::default());
        crate::ui::persistence::tests::settle(fixture.handle, &mut fixture.visual);
        fixture.draw();
        fixture.visual.simulate_keystrokes("x");
        fixture
            .handle
            .update(&mut fixture.visual, |app, window, cx| {
                assert_eq!(TerminalPalette::current(cx), palette);
                assert_eq!(app.store.terminal_theme(), palette);
                let settings = app.settings.as_ref().unwrap();
                assert!(settings.error.is_none());
                assert_eq!(app.tabs.len(), 1);
                assert_eq!(app.tabs[0].id, "settings-test-terminal");
                assert_eq!(app.tabs[0].state, ssh::State::Connected);
                let TabContent::Terminal(terminal) = &app.tabs[0].content else {
                    panic!("Settings replaced the remote terminal");
                };
                assert_eq!(terminal.entity_id(), fixture.terminal);
                assert!(!terminal.focus_handle(cx).is_focused(window));
            })
            .unwrap();
        assert_eq!(
            Store::load(&fixture.path).unwrap().terminal_theme(),
            palette
        );
        assert_eq!(fixture.data.load(Ordering::Relaxed), 0);
    }

    fixture.visual.simulate_event(ScrollWheelEvent {
        position: point(px(712.), px(400.)),
        delta: ScrollDelta::Lines(point(0., -100.)),
        ..Default::default()
    });
    fixture.draw();
    let preview = fixture.visual.debug_bounds("settings-preview").unwrap();
    assert!(preview.top() >= px(0.) && preview.bottom() <= px(480.));
    let back = fixture
        .visual
        .debug_bounds("settings-back")
        .unwrap()
        .center();
    fixture.visual.simulate_click(back, Default::default());
    fixture.draw();
    fixture
        .handle
        .update(&mut fixture.visual, |app, window, cx| {
            assert!(app.settings.is_none());
            assert_eq!(app.connection_search.read(cx).value.as_str(), "Production");
            assert_eq!(app.active, Some(0));
            assert!(app.tabs[0].visible_focus(cx).is_focused(window));
        })
        .unwrap();
    assert_eq!(fixture.data.load(Ordering::Relaxed), 0);
    fixture.open_settings();
    fixture.visual.simulate_keystrokes("escape");
    fixture
        .handle
        .read_with(&fixture.visual, |app, _| assert!(app.settings.is_none()))
        .unwrap();
}

#[gpui::test]
fn settings_save_failure_retains_previous_palette_and_reports_error(cx: &mut TestAppContext) {
    let mut fixture = fixture(cx);
    fixture.open_settings();
    std::fs::remove_file(&fixture.path).unwrap();
    std::fs::create_dir(&fixture.path).unwrap();
    let position = fixture
        .visual
        .debug_bounds("settings-palette-Carbon")
        .unwrap()
        .center();
    fixture.visual.simulate_click(position, Default::default());
    crate::ui::persistence::tests::settle(fixture.handle, &mut fixture.visual);
    fixture
        .handle
        .read_with(&fixture.visual, |app, cx| {
            assert_eq!(TerminalPalette::current(cx), TerminalPalette::Classic);
            assert_eq!(app.store.terminal_theme(), TerminalPalette::Classic);
            assert!(
                app.settings
                    .as_ref()
                    .unwrap()
                    .error
                    .as_ref()
                    .unwrap()
                    .starts_with("Cannot save terminal theme:")
            );
        })
        .unwrap();
}
