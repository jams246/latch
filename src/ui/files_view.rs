use super::{
    button,
    files::{Files, pane::Side},
};
use gpui::{prelude::*, *};

impl Render for Files {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for (index, side) in [Side::Local, Side::Remote].into_iter().enumerate() {
            if std::mem::take(&mut self.breadcrumb_reveal[index]) {
                self.scroll_breadcrumbs(side);
                let entity = cx.weak_entity();
                // GPUI updates the scroll viewport during prepaint.
                window.on_next_frame(move |_, cx| {
                    let _ = entity.update(cx, |this, cx| {
                        this.scroll_breadcrumbs(side);
                        cx.notify();
                    });
                });
            }
        }
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .p_3()
            .gap_3()
            .bg(rgb(0x15232f))
            .when(self.transfer.is_some(), |view| {
                view.child(
                    button("cancel-transfer", "Cancel transfer").on_click(cx.listener(
                        |this, _, _, _| {
                            if let Some(transfer) = &this.transfer {
                                transfer.cancel();
                            }
                        },
                    )),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(self.render_files_pane(Side::Local, cx))
                    .child(self.render_files_pane(Side::Remote, cx)),
            )
            .child(self.render_context_menu(window, cx))
            .child(self.render_action_dialog(window, cx))
    }
}

impl Files {
    fn render_files_pane(&self, side: Side, cx: &mut Context<Self>) -> Stateful<Div> {
        let (title, status, busy) = match side {
            Side::Local => ("Local computer", &self.local_status, self.local_busy),
            Side::Remote => ("Connected server", &self.status, self.busy),
        };
        let status = self.action_status[usize::from(side == Side::Remote)]
            .as_ref()
            .unwrap_or(status);
        div()
            .id(side.id())
            .debug_selector(move || format!("{}-pane", side.id()))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap_2()
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(move |this, _, _, cx| this.history_back(side, cx)),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(move |this, _, _, cx| this.history_forward(side, cx)),
            )
            .child(div().text_lg().child(title))
            .child(self.breadcrumbs(side, cx))
            .child(self.address_row(side, cx))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0xa8c1d3))
                    .truncate()
                    .child(if busy {
                        "Loading…".into()
                    } else {
                        status.clone()
                    }),
            )
            .child(
                div()
                    .id("table")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_x_scroll()
                    .child(self.columns(side, cx))
                    .child(self.file_rows(side, cx)),
            )
            .child(div().text_xs().text_color(rgb(0xa8c1d3)).child(match side {
                Side::Local => "Double-click folders or links to open. Dates in UTC.",
                Side::Remote => "\u{a0}",
            }))
    }
}
