use super::Latch;
use gpui::{prelude::*, *};

impl Render for Latch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = super::theme::CARBON;
        let collapsed = self.sidebar_collapsed && self.settings.is_none();
        let (sidebar, body) = if self.settings.is_some() {
            (self.settings_sidebar(cx), self.settings_page(cx))
        } else {
            let tabs = self.tab_bar(window, cx);
            let content = self.workspace(window, cx);
            (
                self.connections_sidebar(cx),
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .child(tabs)
                    .children(
                        self.error
                            .clone()
                            .filter(|_| self.editor.is_none())
                            .map(|error| div().p_3().text_color(rgb(colors.danger)).child(error)),
                    )
                    .child(content),
            )
        };
        div()
            .id("latch")
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && this.tab_drag.origin.is_some() {
                    this.cancel_tab_drag(cx);
                    cx.stop_propagation();
                }
            }))
            .relative()
            .size_full()
            .flex()
            .bg(rgb(colors.background))
            .text_color(rgb(colors.text))
            .text_size(px(14.))
            .font_family("Segoe UI")
            .child(
                div()
                    .id("sidebar")
                    .debug_selector(|| "sidebar".into())
                    .flex()
                    .flex_col()
                    .w(px(if collapsed { 60. } else { 310. }))
                    .flex_shrink_0()
                    .p_3()
                    .gap_3()
                    .bg(rgb(colors.panel))
                    .border_r_1()
                    .border_color(rgb(colors.border))
                    .child(sidebar),
            )
            .child(
                div()
                    .debug_selector(|| "workspace".into())
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(body),
            )
            .child(self.tab_menu_view(cx))
            .when(self.editor.is_some(), |view| {
                view.child(self.editor_view(cx))
            })
            .when(self.connection_delete.is_some(), |view| {
                view.child(self.connection_delete_view(cx))
            })
            .child(self.host_dialog_view(window, cx))
    }
}
