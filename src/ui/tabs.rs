use super::{Latch, button};
use gpui::{prelude::*, *};

#[cfg(test)]
mod scroll_tests;

#[derive(Default)]
pub(super) struct TabDrag {
    origin: Option<(Point<Pixels>, Pixels)>,
    did_drag: bool,
}

impl Latch {
    fn drag_tabs(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.tab_drag.origin = None;
            return;
        }
        let Some((origin, offset)) = self.tab_drag.origin else {
            return;
        };
        let delta = event.position - origin;
        if delta.magnitude() > 4. {
            self.tab_drag.did_drag = true;
        }
        if self.tab_drag.did_drag {
            let x = (offset + delta.x).clamp(-self.tab_scroll.max_offset().width, px(0.));
            self.tab_scroll.set_offset(point(x, px(0.)));
            cx.stop_propagation();
            cx.notify();
        }
    }

    pub(super) fn tab_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = super::theme::CARBON;
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
        div()
            .id("tabs")
            .debug_selector(|| "tabs".into())
            .relative()
            .flex()
            .flex_shrink_0()
            .min_w_0()
            .gap_1()
            .p_2()
            .overflow_x_scroll()
            .track_scroll(&self.tab_scroll)
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, _| {
                if event.button == MouseButton::Left {
                    this.tab_drag.origin = Some((event.position, this.tab_scroll.offset().x));
                    this.tab_drag.did_drag = false;
                }
            }))
            .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(if self.active == Some(index) {
                        colors.accent
                    } else {
                        colors.border
                    }))
                    .bg(rgb(if self.active == Some(index) {
                        colors.selected
                    } else {
                        colors.control
                    }))
                    .child(
                        button(SharedString::from(format!("close-{}", tab.id)), "×")
                            .debug_selector(|| format!("tab-close-{index}"))
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(32.))
                            .p_0()
                            .bg(rgba(0))
                            .border_color(rgba(0))
                            .text_size(px(22.))
                            .text_color(rgb(colors.danger))
                            .on_click(cx.listener(move |this, event, window, cx| {
                                if this.tab_drag.did_drag && matches!(event, ClickEvent::Mouse(_)) {
                                    return;
                                }
                                this.close_tab(index, window, cx);
                            })),
                    )
                    .child(
                        button(
                            SharedString::from(format!("tab-{}", tab.id)),
                            tab.name.clone(),
                        )
                        .debug_selector(|| format!("tab-name-{index}"))
                        .px(px(10.))
                        .bg(rgba(0))
                        .border_color(rgba(0))
                        .on_click(cx.listener(
                            move |this, event, window, cx| {
                                if this.tab_drag.did_drag && matches!(event, ClickEvent::Mouse(_)) {
                                    return;
                                }
                                this.active = Some(index);
                                if let Some(tab) = this.tabs.get(index) {
                                    window.focus(&tab.visible_focus(cx));
                                }
                                cx.notify();
                            },
                        )),
                    )
            }))
            .child(
                // Keep tracking a tab drag when the pointer leaves the strip.
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        let moves = entity.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture {
                                moves.update(cx, |this, cx| this.drag_tabs(event, cx));
                            }
                        });
                        let releases = entity.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture && event.button == MouseButton::Left
                            {
                                releases.update(cx, |this, _| this.tab_drag.origin = None);
                            }
                        });
                    },
                )
                .absolute()
                .size(px(0.)),
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
