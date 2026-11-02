use super::Page;
use crate::ui::{
    Latch,
    controls::{icon, quiet},
    theme::current,
};
use gpui::{prelude::*, *};

impl Latch {
    pub(in crate::ui) fn settings_workspace(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let colors = current();
        let Some(settings) = self.settings.as_ref() else {
            return div();
        };
        let page = settings.page;
        let narrow = window.viewport_size().width < px(560.);
        let nav = div()
            .flex()
            .flex_shrink_0()
            .p_2()
            .gap_1()
            .bg(rgb(colors.panel))
            .when(!narrow, |d| {
                d.flex_col()
                    .w(px(178.))
                    .border_r_1()
                    .border_color(rgb(colors.border))
            })
            .children(
                [
                    (
                        Page::Appearance,
                        "Appearance",
                        "settings-terminal-theme",
                        "\u{e790}",
                    ),
                    (Page::Data, "Data", "settings-data", "\u{ed41}"),
                    (Page::About, "About", "settings-about", "\u{e946}"),
                ]
                .into_iter()
                .map(|(target, label, id, glyph)| {
                    quiet(id, "")
                        .debug_selector(move || id.into())
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_2()
                        .child(icon(glyph))
                        .child(label)
                        .when(page == target, |b| {
                            b.bg(rgb(colors.selected)).text_color(rgb(colors.accent))
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.data_busy || this.shutting_down {
                                return;
                            }
                            if let Some(s) = &mut this.settings {
                                s.page = target;
                                window.focus(&s.focus);
                                cx.notify()
                            }
                        }))
                }),
            );
        let content = match page {
            Page::Appearance => self.appearance_page(cx).into_any_element(),
            Page::Data => self.settings_data_page(cx).into_any_element(),
            Page::About => self.settings_about_page(cx).into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .track_focus(&settings.focus)
            .capture_key_down(cx.listener(Self::settings_key_down))
            .child(
                div()
                    .h(px(60.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(colors.border))
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Settings"),
                    )
                    .child(
                        quiet("settings-back", "‹  Back to workspace")
                            .debug_selector(|| "settings-back".into())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.close_settings(window, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .when(narrow, |d| d.flex_col())
                    .child(nav)
                    .child(div().flex_1().min_h_0().min_w_0().child(content)),
            )
    }
}
