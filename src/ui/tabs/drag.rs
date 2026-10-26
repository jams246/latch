use crate::ui::Latch;
use gpui::*;
use std::time::Duration;

#[derive(Default)]
pub(in crate::ui) struct TabDrag {
    pub(in crate::ui) origin: Option<(Point<Pixels>, Pixels)>,
    pub(in crate::ui) did_drag: bool,
    pub(super) moving: Option<String>,
    pub(super) target: Option<WindowHandle<Latch>>,
    pub(super) preview: Option<super::preview::Preview>,
    pub(super) preview_failed: bool,
    monitor: Option<Task<()>>,
}

impl TabDrag {
    pub(in crate::ui) fn dragging(&self, id: &str) -> bool {
        self.did_drag && self.moving.as_deref() == Some(id)
    }
}

impl Latch {
    pub(in crate::ui) fn begin_tab_move(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.movable_tab(&id) {
            return;
        }
        self.tab_drag.moving = Some(id);
        self.tab_drag.monitor = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                Timer::after(Duration::from_millis(25)).await;
                let keep = this
                    .update_in(cx, |this, window, cx| {
                        if !super::native::captured(window)
                            || !this
                                .tab_drag
                                .moving
                                .as_ref()
                                .is_some_and(|id| this.movable_tab(id))
                        {
                            this.cancel_tab_drag(cx);
                            return false;
                        }
                        this.update_drag_preview(window, cx);
                        true
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
    }

    pub(in crate::ui) fn cancel_tab_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(target) = self.tab_drag.target.take() {
            let _ = target.update(cx, |target, _, cx| {
                target.drop_highlight = false;
                cx.notify();
            });
        }
        self.tab_drag.preview = None;
        self.tab_drag.preview_failed = false;
        self.tab_drag.origin = None;
        self.tab_drag.moving = None;
        self.tab_drag.monitor = None;
        cx.notify();
    }

    pub(in crate::ui) fn drag_tabs(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            self.cancel_tab_drag(cx);
            return;
        }
        let Some((origin, offset)) = self.tab_drag.origin else {
            return;
        };
        let delta = event.position - origin;
        if delta.magnitude() > 4. {
            self.tab_drag.did_drag = true;
        }
        if !self.tab_drag.did_drag {
            return;
        }
        let x = (offset + delta.x).clamp(-self.tab_scroll.max_offset().width, px(0.));
        self.tab_scroll.set_offset(point(x, px(0.)));
        let mut hovered = None;
        if self
            .tab_drag
            .moving
            .as_ref()
            .is_some_and(|id| self.movable_tab(id))
        {
            for handle in self.profile.read(cx).windows.clone() {
                if handle == self.owner {
                    continue;
                }
                let Some(handle) = handle.downcast::<Latch>() else {
                    continue;
                };
                let hit = handle
                    .update(cx, |target, window, cx| {
                        target.accepts_tab()
                            && target.profile == self.profile
                            && super::native::pointer_in(window).is_some_and(|pointer| {
                                target
                                    .tab_bounds
                                    .is_some_and(|bounds| bounds.contains(&pointer))
                            })
                            && cx.windows().contains(&handle.into())
                    })
                    .unwrap_or(false);
                if hit {
                    hovered = Some(handle);
                    break;
                }
            }
        }
        if self.tab_drag.target != hovered {
            if let Some(previous) = self.tab_drag.target.take() {
                let _ = previous.update(cx, |target, _, cx| {
                    target.drop_highlight = false;
                    cx.notify();
                });
            }
            if let Some(target) = hovered {
                let _ = target.update(cx, |target, _, cx| {
                    target.drop_highlight = true;
                    cx.notify();
                });
            }
            self.tab_drag.target = hovered;
        }
        self.update_drag_preview(window, cx);
        cx.stop_propagation();
        cx.notify();
    }

    pub(in crate::ui) fn finish_tab_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab_drag.did_drag
            && let (Some(id), Some(target)) = (self.tab_drag.moving.clone(), self.tab_drag.target)
        {
            let valid = target
                .update(cx, |target, window, _| {
                    super::native::pointer_in(window).is_some_and(|pointer| {
                        target
                            .tab_bounds
                            .is_some_and(|bounds| bounds.contains(&pointer))
                    })
                })
                .unwrap_or(false);
            if valid {
                self.move_tab_to(&id, target, window, cx);
            }
        }
        self.cancel_tab_drag(cx);
    }
}
