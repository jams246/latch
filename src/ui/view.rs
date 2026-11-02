use super::{
    Latch,
    controls::{icon, quiet},
    theme::current,
};
use gpui::{prelude::*, *};

impl Render for Latch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = current();
        let tab_bar = self.tab_bar(window, cx);
        let content = if self.settings.is_some() {
            self.settings_workspace(window, cx).into_any_element()
        } else if self.active.is_none() {
            self.connection_library(window, cx).into_any_element()
        } else {
            self.workspace(window, cx)
        };
        let library = quiet("library", "Connections")
            .border_0()
            .debug_selector(|| "library-tab".into())
            .h_full()
            .flex()
            .items_center()
            .text_sm()
            .when(self.active.is_none() && self.settings.is_none(), |b| {
                b.bg(rgb(colors.background))
                    .border_b_2()
                    .border_color(rgb(colors.accent))
            })
            .on_click(cx.listener(|this, _, window, cx| {
                if this.data_busy || this.shutting_down {
                    return;
                }
                this.settings = None;
                this.active = None;
                this.dismiss_tab_menu(window, cx);
                window.focus(&this.connection_search.read(cx).focus);
                cx.notify();
            }));
        let settings = quiet("settings-open", "")
            .debug_selector(|| "settings-open".into())
            .flex()
            .items_center()
            .gap_2()
            .child(icon("\u{e713}"))
            .child("Settings")
            .when(self.settings.is_some(), |b| b.bg(rgb(colors.control)))
            .on_click(cx.listener(|this, _, window, cx| {
                if this.settings.is_some() {
                    this.close_settings(window, cx)
                } else {
                    this.open_settings(window, cx)
                }
            }));
        div()
            .id("latch")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(colors.background))
            .text_color(rgb(colors.text))
            .text_size(px(14.))
            .font_family("Segoe UI")
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    if this.popup.is_some() {
                        this.dismiss_choices(window, cx);
                        cx.stop_propagation()
                    } else if this.tab_drag.origin.is_some() {
                        this.cancel_tab_drag(cx);
                        cx.stop_propagation()
                    } else if this.settings.is_some()
                        && !this.tabs.iter().any(|t| t.host_dialog.is_some())
                    {
                        this.close_settings(window, cx);
                        cx.stop_propagation()
                    }
                }
            }))
            .child(
                div()
                    .flex()
                    .h(px(46.))
                    .flex_shrink_0()
                    .items_center()
                    .px_2()
                    .gap_1()
                    .bg(rgb(colors.panel))
                    .border_b_1()
                    .border_color(rgb(colors.border))
                    .child(library)
                    .child(div().min_w_0().flex_1().child(tab_bar))
                    .child(settings),
            )
            .children(
                self.error
                    .clone()
                    .filter(|_| self.editor.is_none() && self.settings.is_none())
                    .map(|e| div().px_3().py_1().text_color(rgb(colors.danger)).child(e)),
            )
            .child(
                div()
                    .debug_selector(|| "workspace".into())
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(content),
            )
            .child(self.choices_view(cx))
            .child(self.tab_menu_view(cx))
            .when(self.editor.is_some(), |v| v.child(self.editor_view(cx)))
            .when(self.connection_delete.is_some(), |v| {
                v.child(self.connection_delete_view(cx))
            })
            .child(self.host_dialog_view(window, cx))
    }
}
