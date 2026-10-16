use super::files::{Files, pane::Side};
use crate::ui::theme::CARBON;
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
            .bg(rgb(CARBON.background))
            .text_color(rgb(CARBON.text))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(self.render_files_pane(Side::Local, cx))
                    .child(self.render_files_pane(Side::Remote, cx)),
            )
            .when(self.transfer.is_some(), |view| {
                view.child(self.operations_bar(cx))
            })
            .child(self.render_context_menu(window, cx))
            .child(self.render_action_dialog(window, cx))
            .child(self.render_conflict_dialog(window, cx))
    }
}

impl Files {
    fn render_files_pane(&self, side: Side, cx: &mut Context<Self>) -> Stateful<Div> {
        let title = match side {
            Side::Local => "Local computer",
            Side::Remote => self.connection_name.as_str(),
        };
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
            .child(
                div()
                    .id("pane-title")
                    .debug_selector(move || format!("{}-title", side.id()))
                    .text_lg()
                    .truncate()
                    .child(title.to_owned()),
            )
            .child(self.breadcrumbs(side, cx))
            .child(self.address_row(side, cx))
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
    }
}
