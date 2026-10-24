use std::sync::{Arc, mpsc};

use gpui::{
    Modifiers, ScrollDelta, TestAppContext, VisualTestContext, WindowHandle, point, px, size,
};

use super::*;
use crate::terminal::{TerminalInput, TerminalState};

#[path = "wheel_tests.rs"]
mod wheel_tests;

struct Fixture {
    handle: WindowHandle<TerminalView>,
    visual: VisualTestContext,
    state: TerminalState,
    sent: mpsc::Receiver<TerminalInput>,
    text: String,
}

impl Fixture {
    fn new(cx: &mut TestAppContext) -> Self {
        let state = TerminalState::new(40, 10);
        let (send, sent) = mpsc::channel();
        let handle = cx.add_window(|_, cx| {
            TerminalView::new(
                state.clone(),
                Arc::new(move |input| send.send(input).unwrap()),
                cx,
            )
        });
        let mut visual = VisualTestContext::from_window(*handle, cx);
        visual.simulate_resize(size(px(400.), px(220.)));
        visual.update(|window, cx| window.draw(cx)).clear();
        let text = (0..100)
            .map(|line| format!("line {line:03}"))
            .collect::<Vec<_>>()
            .join("\n");
        state.feed(text.replace('\n', "\r\n").as_bytes());
        Self {
            handle,
            visual,
            state,
            sent,
            text,
        }
    }

    fn cell(&self, row: f32, column: f32) -> Point<Pixels> {
        self.handle
            .read_with(&self.visual, |view, _| {
                let bounds = view.bounds.unwrap();
                point(
                    bounds.left() + view.cell_width * (column + 0.5),
                    bounds.top() + paint::LINE_HEIGHT * (row + 0.5),
                )
            })
            .unwrap()
    }

    fn wheel(&mut self, position: Point<Pixels>, delta: ScrollDelta) {
        self.visual.simulate_event(ScrollWheelEvent {
            position,
            delta,
            ..Default::default()
        });
    }

    fn autoscroll(&mut self) {
        self.handle
            .update(&mut self.visual, |view, _, cx| {
                view.autoscroll_selection(cx)
            })
            .unwrap();
    }

    fn first_line(&self) -> String {
        self.state
            .snapshot()
            .cells
            .iter()
            .take(8)
            .map(|cell| cell.cell.c)
            .collect()
    }

    fn data(&self) -> Vec<u8> {
        self.sent
            .try_iter()
            .filter_map(|input| match input {
                TerminalInput::Data(bytes) => Some(bytes),
                TerminalInput::Resize { .. } => None,
            })
            .flatten()
            .collect()
    }
}

#[gpui::test]
fn small_pixel_wheel_events_accumulate_into_scrollback(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    let position = fixture.cell(5., 2.);
    let before: String = fixture.first_line();
    for _ in 0..4 {
        fixture.wheel(position, ScrollDelta::Pixels(point(px(0.), px(5.))));
    }
    let after: String = fixture.first_line();
    assert_eq!((before.as_str(), after.as_str()), ("line 090", "line 089"));
}

#[gpui::test]
fn reversing_wheel_direction_discards_old_fractional_motion(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.scroll(10);
    let position = fixture.cell(5., 2.);
    fixture.wheel(position, ScrollDelta::Lines(point(0., 0.75)));
    fixture.wheel(position, ScrollDelta::Lines(point(0., -1.)));
    let row: String = fixture.first_line();
    assert_eq!(row, "line 081");
}

#[gpui::test]
fn wheel_extends_selection_from_history_top_to_bottom_and_copies_all_text(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.scroll(i32::MAX);
    let start = fixture.cell(0., 0.);
    let end = fixture.cell(9., 7.);
    fixture
        .visual
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    fixture
        .visual
        .simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    fixture.wheel(end, ScrollDelta::Lines(point(0., -50.)));
    fixture.wheel(end, ScrollDelta::Lines(point(0., -40.)));
    fixture
        .visual
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    fixture.visual.simulate_keystrokes("ctrl-shift-c");
    let copied = fixture
        .visual
        .update(|_, cx| cx.read_from_clipboard().and_then(|item| item.text()));
    assert_eq!(copied.as_deref(), Some(fixture.text.as_str()));
}

#[gpui::test]
fn reverse_wheel_selection_reaches_oldest_history(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    let start = fixture.cell(9., 7.);
    let end = fixture.cell(0., 0.);
    fixture
        .visual
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    fixture
        .visual
        .simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    fixture.wheel(end, ScrollDelta::Lines(point(0., 90.)));
    assert_eq!(
        fixture.state.selected_text().as_deref(),
        Some(fixture.text.as_str())
    );
}

#[gpui::test]
fn completed_selection_survives_wheel_scrolling(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    let start = fixture.cell(0., 0.);
    let end = fixture.cell(0., 7.);
    fixture
        .visual
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    fixture
        .visual
        .simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    fixture
        .visual
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    fixture.wheel(end, ScrollDelta::Lines(point(0., 20.)));
    assert_eq!(fixture.state.selected_text().as_deref(), Some("line 090"));
}

#[gpui::test]
fn dragging_below_viewport_scrolls_until_release_outside(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.scroll(i32::MAX);
    let start = fixture.cell(0., 0.);
    let outside = fixture.cell(10., 7.);
    fixture
        .visual
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    fixture
        .visual
        .simulate_mouse_move(outside, Some(MouseButton::Left), Modifiers::default());
    for _ in 0..50 {
        fixture.autoscroll();
    }
    assert_eq!(
        fixture.state.selected_text().as_deref(),
        Some(fixture.text.as_str())
    );
    fixture
        .visual
        .simulate_mouse_up(outside, MouseButton::Left, Modifiers::default());
    fixture.state.scroll(10);
    fixture.autoscroll();
    let row: String = fixture.first_line();
    assert_eq!(row, "line 080");
}

#[gpui::test]
fn alternate_screen_wheel_preserves_application_keys_and_selection(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.feed(b"\x1b[?1049h\x1b[H\x1b[?1hhello");
    let start = fixture.cell(0., 0.);
    let end = fixture.cell(0., 4.);
    fixture.wheel(end, ScrollDelta::Lines(point(0., 2.)));
    assert_eq!(fixture.data(), b"\x1bOA\x1bOA");
    fixture
        .visual
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    fixture
        .visual
        .simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    fixture.wheel(end, ScrollDelta::Lines(point(0., -2.)));
    assert!(fixture.data().is_empty());
    assert_eq!(fixture.state.selected_text().as_deref(), Some("hello"));
}

#[gpui::test]
fn paste_returns_to_live_output_and_uses_bracketed_paste(cx: &mut TestAppContext) {
    let mut fixture = Fixture::new(cx);
    fixture.state.feed(b"\x1b[?2004h");
    fixture.state.scroll(90);
    let position = fixture.cell(5., 2.);
    fixture
        .visual
        .simulate_click(position, Modifiers::default());
    fixture
        .visual
        .update(|_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string("hello".into())));
    fixture.visual.simulate_keystrokes("ctrl-shift-v");
    assert_eq!(fixture.data(), b"\x1b[200~hello\x1b[201~");
    let row: String = fixture.first_line();
    assert_eq!(row, "line 090");
}
