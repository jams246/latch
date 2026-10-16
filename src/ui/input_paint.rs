use super::input::Input;
use gpui::{prelude::*, *};

const LINE_HEIGHT: Pixels = px(20.);

impl Input {
    fn layout_text(&mut self, bounds: Bounds<Pixels>, window: &mut Window) {
        let palette = crate::ui::theme::CARBON;
        let text = if self.masked {
            "•".repeat(self.value.chars().count())
        } else {
            self.value.to_string()
        };
        let run = TextRun {
            len: text.len(),
            font: window.text_style().font(),
            color: rgb(if self.disabled {
                palette.muted
            } else {
                palette.text
            })
            .into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = window
            .text_system()
            .shape_line(text.into(), px(14.), &[run], None);
        let cursor = line.x_for_index(self.display_offset(self.cursor()));
        let available = (bounds.size.width - px(2.)).max(px(1.));
        self.scroll = self.scroll.min((line.width - available).max(px(0.)));
        if cursor < self.scroll {
            self.scroll = cursor;
        }
        if cursor > self.scroll + available {
            self.scroll = cursor - available;
        }
        self.bounds = Some(Bounds::new(
            point(
                bounds.left(),
                bounds.top() + ((bounds.size.height - LINE_HEIGHT) / 2.).max(px(0.)),
            ),
            size(bounds.size.width, LINE_HEIGHT),
        ));
        self.layout = Some(line);
    }

    fn paint_text(&self, window: &mut Window, cx: &mut App) {
        let (Some(bounds), Some(line)) = (self.bounds, self.layout.as_ref()) else {
            return;
        };
        let focused = self.focus.is_focused(window) && !self.disabled;
        let palette = crate::ui::theme::CARBON;
        let origin = point(bounds.left() - self.scroll, bounds.top());
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if focused && !self.selection.is_empty() {
                let start = line.x_for_index(self.display_offset(self.selection.start));
                let end = line.x_for_index(self.display_offset(self.selection.end));
                window.paint_quad(fill(
                    Bounds::new(
                        point(origin.x + start, origin.y),
                        size(end - start, LINE_HEIGHT),
                    ),
                    rgb(palette.selected),
                ));
            }
            let _ = line.paint(origin, LINE_HEIGHT, window, cx);
            if focused {
                let cursor = line.x_for_index(self.display_offset(self.cursor()));
                window.paint_quad(fill(
                    Bounds::new(
                        point(origin.x + cursor, origin.y),
                        size(px(1.5), LINE_HEIGHT),
                    ),
                    rgb(palette.accent),
                ));
            }
        });
    }
}

impl Render for Input {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = crate::ui::theme::CARBON;
        let entity = cx.entity();
        let painted = entity.clone();
        let focus = self.focus.clone().tab_index(0).tab_stop(!self.disabled);
        div()
            .id("field")
            .track_focus(&focus)
            .w_full()
            .h(px(36.))
            .px_2()
            .py_1()
            .overflow_hidden()
            .bg(rgb(palette.background))
            .border_1()
            .border_color(rgb(if self.focus.is_focused(window) && !self.disabled {
                palette.accent
            } else {
                palette.border
            }))
            .rounded_md()
            .cursor(if self.disabled {
                CursorStyle::Arrow
            } else {
                CursorStyle::IBeam
            })
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if this.disabled {
                        return;
                    }
                    window.focus(&this.focus);
                    this.dragging = true;
                    this.select_to(
                        this.offset_for_point(event.position),
                        event.modifiers.shift,
                        cx,
                    );
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.dragging && event.dragging() && !this.disabled {
                    this.select_to(this.offset_for_point(event.position), true, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .child(
                canvas(
                    move |bounds, window, cx| {
                        entity.update(cx, |this, _| this.layout_text(bounds, window))
                    },
                    move |bounds, _, window, cx| {
                        painted.update(cx, |this, cx| {
                            window.handle_input(
                                &this.focus,
                                ElementInputHandler::new(bounds, painted.clone()),
                                cx,
                            );
                            this.paint_text(window, cx);
                        });
                    },
                )
                .size_full(),
            )
    }
}
