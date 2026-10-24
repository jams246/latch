use super::Page;
use crate::ui::{Latch, button, theme::CARBON};
use gpui::{prelude::*, *};
use latch::terminal::TerminalPalette;

impl Latch {
    pub(in crate::ui) fn settings_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let page = self.settings.as_ref().map(|settings| settings.page);
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .gap_3()
            .on_key_down(cx.listener(Self::settings_key_down))
            .child(div().text_lg().child("Settings"))
            .child(
                button("settings-terminal-theme", "Terminal theme")
                    .debug_selector(|| "settings-terminal-theme".into())
                    .w_full()
                    .when(page == Some(Page::Terminal), |button| {
                        button
                            .bg(rgb(CARBON.selected))
                            .border_color(rgb(CARBON.accent))
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        if !this.data_busy
                            && !this.shutting_down
                            && let Some(settings) = &mut this.settings
                        {
                            settings.page = Page::Terminal;
                            window.focus(&settings.focus);
                            cx.notify();
                        }
                    })),
            )
            .child(
                button("settings-data", "Data")
                    .debug_selector(|| "settings-data".into())
                    .w_full()
                    .when(page == Some(Page::Data), |button| {
                        button
                            .bg(rgb(CARBON.selected))
                            .border_color(rgb(CARBON.accent))
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        if !this.data_busy
                            && !this.shutting_down
                            && let Some(settings) = &mut this.settings
                        {
                            settings.page = Page::Data;
                            window.focus(&settings.focus);
                            cx.notify();
                        }
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(CARBON.muted))
                    .child(concat!("Latch ", env!("CARGO_PKG_VERSION"))),
            )
            .child(
                button("settings-back", "Back to connections")
                    .debug_selector(|| "settings-back".into())
                    .w_full()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_settings(window, cx);
                    })),
            )
    }

    pub(in crate::ui) fn settings_page(&self, cx: &mut Context<Self>) -> Div {
        let Some(settings) = &self.settings else {
            return div();
        };
        if settings.page == Page::Data {
            return self.settings_data_page(cx);
        }
        let active = TerminalPalette::current(cx);
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .track_focus(&settings.focus)
            .capture_key_down(cx.listener(Self::settings_key_down))
            .child(
                div()
                    .id("settings-scroll")
                    .debug_selector(|| "settings-page".into())
                    .flex_1()
                    .min_h_0()
                    .p_4()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(div().text_2xl().child("Terminal theme"))
                            .child(
                                div()
                                    .text_color(rgb(CARBON.muted))
                                    .child("Applies to every terminal tab. Your choice is saved."),
                            )
                            .child(div().flex().flex_wrap().gap_2().children(
                                TerminalPalette::ALL.into_iter().map(|palette| {
                                    let colors = palette.colors();
                                    button(
                                        SharedString::from(format!(
                                            "settings-palette-{}",
                                            palette.name()
                                        )),
                                        palette.name(),
                                    )
                                    .debug_selector(move || {
                                        format!("settings-palette-{}", palette.name())
                                    })
                                    .w(px(164.))
                                    .p_2()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .when(active == palette, |card| {
                                        card.bg(rgb(CARBON.selected))
                                            .border_color(rgb(CARBON.accent))
                                    })
                                    .child(div().flex().gap_1().children(
                                        colors.ansi[..8].iter().map(|color| {
                                            div().size(px(12.)).rounded_sm().bg(rgb(*color))
                                        }),
                                    ))
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.choose_terminal_theme(palette, cx);
                                        },
                                    ))
                                }),
                            ))
                            .children(settings.error.iter().map(|error| {
                                div().text_color(rgb(CARBON.danger)).child(error.clone())
                            }))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Live preview"),
                            )
                            .child(
                                div()
                                    .id("settings-preview")
                                    .debug_selector(|| "settings-preview".into())
                                    .h(px(180.))
                                    .flex_shrink_0()
                                    .border_1()
                                    .border_color(rgb(CARBON.border))
                                    .child(settings.preview.clone()),
                            ),
                    ),
            )
    }
}
