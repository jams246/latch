use super::Latch;
use gpui::{prelude::*, *};

pub(super) mod menu;
#[cfg(test)]
mod scroll_tests;

mod drag;
mod drag_visuals;
mod header;
mod movement;
pub(super) mod native;
mod preview;
pub(super) use drag::TabDrag;

impl Latch {
    pub(super) fn tab_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = super::theme::current();
        let reveal_key = self.active.and_then(|index| {
            self.tabs
                .get(index)
                .map(|tab| (tab.id.clone(), index, window.viewport_size().width))
        });
        if self.tab_reveal_key != reveal_key {
            if let Some((_, index, _)) = &reveal_key {
                let index = *index;
                self.tab_scroll.scroll_to_item(index);
                let entity = cx.weak_entity();
                // GPUI's scroll handle receives the resized viewport during prepaint.
                window.on_next_frame(move |_, cx| {
                    let _ = entity.update(cx, |this, cx| {
                        if let Some(index) = this.active {
                            this.tab_scroll.scroll_to_item(index);
                            cx.notify();
                        }
                    });
                });
            }
            self.tab_reveal_key = reveal_key;
        }
        let entity = cx.entity();
        let strip = div()
            .id("tabs")
            .debug_selector(|| "tabs".into())
            .relative()
            .flex()
            .flex_shrink_0()
            .min_w_0()
            .gap_1()
            .h(px(44.))
            .when(self.drop_highlight, |strip| {
                strip.border_b_2().border_color(rgb(colors.accent))
            })
            .overflow_x_scroll()
            .track_scroll(&self.tab_scroll)
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, _| {
                if event.button == MouseButton::Left {
                    this.tab_drag.origin = Some((event.position, this.tab_scroll.offset().x));
                    this.tab_drag.did_drag = false;
                }
            }))
            .children((0..self.tabs.len()).map(|index| self.tab_header(index, window, cx)));
        div()
            .relative()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .child(strip)
            .child(
                // Keep tracking a tab drag when the pointer leaves the strip.
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        entity.update(cx, |this, _| this.tab_bounds = Some(bounds));
                        let moves = entity.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture {
                                moves.update(cx, |this, cx| this.drag_tabs(event, window, cx));
                            }
                        });
                        let releases = entity.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                releases.update(cx, |this, cx| this.finish_tab_drag(window, cx));
                            }
                        });
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .into_any_element()
    }
}

pub(super) fn active_after_close(
    active: Option<usize>,
    removed: usize,
    remaining: usize,
) -> Option<usize> {
    active.and_then(|active| {
        if remaining == 0 {
            None
        } else if active > removed {
            Some(active - 1)
        } else if active == removed {
            Some(removed.min(remaining - 1))
        } else {
            Some(active)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::active_after_close;

    #[test]
    fn closing_a_tab_preserves_active_identity_or_focuses_its_neighbor() {
        for (active, removed, remaining, expected) in [
            (None, 0, 0, None),
            (Some(0), 0, 0, None),
            (Some(2), 0, 2, Some(1)),
            (Some(0), 2, 2, Some(0)),
            (Some(1), 1, 2, Some(1)),
            (Some(2), 2, 2, Some(1)),
        ] {
            assert_eq!(active_after_close(active, removed, remaining), expected);
        }
    }
}
