use std::{sync::Arc, time::Duration};

use alacritty_terminal::vte::ansi::{Color, NamedColor};
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, FocusHandle, Focusable, KeyDownEvent,
    MouseButton, Pixels, Point, Render, Task, Timer, Window, canvas, div, prelude::*, px, rgb,
};

use super::{TerminalPalette, TerminalState, colors, input, paint};

#[path = "mouse.rs"]
mod mouse;

/// User input and size changes to forward to the tab's SSH session.
pub enum TerminalInput {
    /// UTF-8 text or terminal key escape sequences.
    Data(Vec<u8>),
    /// The remote PTY's new character dimensions.
    Resize {
        /// Number of character columns.
        columns: u16,
        /// Number of visible rows.
        rows: u16,
    },
}

/// Native terminal display with selection, clipboard, scrolling, and IME input.
pub struct TerminalView {
    pub(super) state: TerminalState,
    pub(super) sink: Arc<dyn Fn(TerminalInput) + Send + Sync>,
    pub(super) focus: FocusHandle,
    pub(super) bounds: Option<Bounds<Pixels>>,
    pub(super) cell_width: Pixels,
    pub(super) composition: String,
    revision: u64,
    wheel_remainder: f32,
    selection_position: Option<Point<Pixels>>,
    selection_task: Option<Task<()>>,
}

impl TerminalView {
    /// Attach a terminal to a nonblocking session input callback.
    pub fn new(
        state: TerminalState,
        sink: Arc<dyn Fn(TerminalInput) + Send + Sync>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_millis(33)).await;
                if this
                    .update(cx, |this, cx| {
                        let revision = this.state.revision();
                        if revision != this.revision {
                            this.revision = revision;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            state,
            sink,
            focus: cx.focus_handle(),
            bounds: None,
            cell_width: px(8.4),
            composition: String::new(),
            revision: 0,
            wheel_remainder: 0.,
            selection_position: None,
            selection_task: None,
        }
    }

    pub(super) fn send(&self, bytes: Vec<u8>) {
        self.state.scroll_bottom();
        (self.sink)(TerminalInput::Data(bytes));
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let modifiers = key.modifiers;
        if input::is_clipboard_shortcut(key) {
            match key.key.as_str() {
                "c" => {
                    if let Some(text) = self.state.selected_text() {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.send(input::paste(&text, self.state.mode()));
                    }
                }
                _ => return,
            }
        } else if modifiers.shift && matches!(key.key.as_str(), "pageup" | "pagedown") {
            let rows = self.state.snapshot().rows.saturating_sub(1) as i32;
            self.state
                .scroll(if key.key == "pageup" { rows } else { -rows });
        } else if let Some(bytes) = input::encode(key, self.state.mode()) {
            self.send(bytes);
        } else {
            return;
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = TerminalPalette::current(cx).colors();
        let entity = cx.entity();
        let paint_entity = entity.clone();
        div()
            .id("terminal")
            .size_full()
            .p(px(10.))
            .overflow_hidden()
            .track_focus(&self.focus)
            .key_context("Terminal")
            .cursor(CursorStyle::IBeam)
            .bg(rgb(colors::resolve(
                Color::Named(NamedColor::Background),
                &self.state.colors(),
                palette,
            )))
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::start_selection))
            .on_scroll_wheel(cx.listener(Self::scroll_wheel))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        entity.update(cx, |this, _| {
                            this.bounds = Some(bounds);
                            this.cell_width = paint::cell_width(window);
                            let columns = (bounds.size.width / this.cell_width)
                                .floor()
                                .clamp(2., u16::MAX as f32)
                                as u16;
                            let rows = (bounds.size.height / paint::LINE_HEIGHT)
                                .floor()
                                .clamp(1., u16::MAX as f32)
                                as u16;
                            if this.state.resize(columns, rows) {
                                (this.sink)(TerminalInput::Resize { columns, rows });
                            }
                            this.state.snapshot()
                        })
                    },
                    move |bounds, snapshot, window, cx| {
                        mouse::track_selection(&paint_entity, window);
                        let (focus, width, composition) = {
                            let view = paint_entity.read(cx);
                            (
                                view.focus.clone(),
                                view.cell_width,
                                view.composition.clone(),
                            )
                        };
                        window.handle_input(
                            &focus,
                            gpui::ElementInputHandler::new(bounds, paint_entity.clone()),
                            cx,
                        );
                        paint::paint(bounds, snapshot, width, &composition, window, cx);
                    },
                )
                .size_full(),
            )
    }
}
