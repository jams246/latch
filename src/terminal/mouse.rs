use std::time::Duration;

use alacritty_terminal::term::TermMode;
use gpui::{
    Context, DispatchPhase, Entity, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, ScrollWheelEvent, Timer, Window,
};

use super::{TerminalView, input, paint};

pub(super) fn track_selection(entity: &Entity<TerminalView>, window: &mut Window) {
    let moves = entity.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture {
            moves.update(cx, |this, cx| {
                if !event.dragging() {
                    this.stop_selection();
                } else if this.selection_position.is_some() {
                    this.selection_position = Some(event.position);
                    this.extend_selection(event.position);
                    cx.notify();
                }
            });
        }
    });
    let releases = entity.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
            releases.update(cx, |this, _| this.stop_selection());
        }
    });
}

impl TerminalView {
    pub(super) fn start_selection(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window);
        let (row, column) = self.cell_at(event.position);
        self.state.select(row, column, true);
        self.selection_position = Some(event.position);
        self.selection_task = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                Timer::after(Duration::from_millis(50)).await;
                let keep_selecting = this.update_in(cx, |this, window, cx| {
                    if !window.is_window_active() || !this.focus.is_focused(window) {
                        this.selection_position = None;
                    }
                    this.autoscroll_selection(cx);
                    this.selection_position.is_some()
                });
                if !keep_selecting.unwrap_or(false) {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn stop_selection(&mut self) {
        self.selection_position = None;
        self.selection_task = None;
    }

    fn cell_at(&self, position: Point<Pixels>) -> (usize, usize) {
        let Some(bounds) = self.bounds else {
            return (0, 0);
        };
        let column = ((position.x - bounds.left()) / self.cell_width).max(0.) as usize;
        let row = ((position.y - bounds.top()) / paint::LINE_HEIGHT).max(0.) as usize;
        (row, column)
    }

    fn extend_selection(&self, position: Point<Pixels>) {
        let (row, column) = self.cell_at(position);
        self.state.select(row, column, false);
    }

    fn autoscroll_selection(&mut self, cx: &mut Context<Self>) {
        let (Some(position), Some(bounds)) = (self.selection_position, self.bounds) else {
            return;
        };
        let lines = if position.y < bounds.top() {
            ((bounds.top() - position.y) / paint::LINE_HEIGHT).ceil() as i32
        } else if position.y >= bounds.bottom() {
            -((position.y - bounds.bottom()) / paint::LINE_HEIGHT + 1.).ceil() as i32
        } else {
            return;
        };
        self.state.scroll(lines.clamp(-10, 10));
        self.extend_selection(position);
        cx.notify();
    }

    pub(super) fn scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(paint::LINE_HEIGHT).y / paint::LINE_HEIGHT;
        if delta != 0. && delta.signum() != self.wheel_remainder.signum() {
            self.wheel_remainder = 0.;
        }
        self.wheel_remainder += delta;
        let lines = self.wheel_remainder.trunc() as i32;
        self.wheel_remainder -= lines as f32;
        if lines != 0 {
            let mode = self.state.mode();
            if mode.contains(TermMode::ALT_SCREEN)
                && !event.modifiers.shift
                && self.selection_position.is_none()
            {
                let key = if lines > 0 { "up" } else { "down" };
                if let Ok(key) = gpui::Keystroke::parse(key) {
                    for _ in 0..lines.unsigned_abs().min(30) {
                        if let Some(bytes) = input::encode(&key, mode) {
                            self.send(bytes);
                        }
                    }
                }
            } else {
                self.state.scroll(lines);
                if self.selection_position.is_some() {
                    self.selection_position = Some(event.position);
                    self.extend_selection(event.position);
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
}

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;
