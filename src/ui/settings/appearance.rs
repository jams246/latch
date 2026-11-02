use crate::ui::{
    Latch, button,
    choices::Choice,
    theme::{current, palette},
};
use gpui::{prelude::*, *};
use latch::{storage::AppTheme, terminal::TerminalPalette};

impl Latch {
    pub(super) fn appearance_page(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(settings) = &self.settings else {
            return div().id("settings-scroll");
        };
        let colors = current();
        let active = TerminalPalette::current(cx);
        let themes = div().flex().gap_2().children(
            [
                (AppTheme::Graphite, "Graphite"),
                (AppTheme::Citron, "Citron"),
            ]
            .into_iter()
            .map(|(theme, name)| {
                let selected = self.store(cx).app_theme() == theme;
                let swatch = palette(theme, WindowAppearance::Dark);
                button(SharedString::from(format!("app-colors-{name}")), "")
                    .debug_selector(move || format!("app-colors-{name}"))
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .border_color(rgb(if selected {
                        colors.accent
                    } else {
                        colors.border
                    }))
                    .child(
                        div().flex().rounded_sm().overflow_hidden().children(
                            [swatch.control, swatch.muted, swatch.accent]
                                .map(|color| div().w(px(8.)).h(px(16.)).bg(rgb(color))),
                        ),
                    )
                    .child(name)
                    .child(if selected { "✓" } else { " " })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.data_busy && !this.shutting_down {
                            this.save_preference(
                                "app colors",
                                move |store| store.set_app_theme(theme),
                                cx,
                            )
                        }
                    }))
            }),
        );
        div()
            .id("settings-scroll")
            .debug_selector(|| "settings-page".into())
            .size_full()
            .overflow_y_scroll()
            .p_4()
            .child(
                div()
                    .max_w(px(640.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Appearance"),
                    )
                    .child(
                        div()
                            .mt_2()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("App colors"),
                    )
                    .child(themes)
                    .child(
                        div()
                            .mt_2()
                            .pt_2()
                            .border_t_1()
                            .border_color(rgb(colors.border))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Terminal theme"),
                            )
                            .child(
                                button("terminal-preset", format!("{}   ▾", active.name()))
                                    .debug_selector(|| "terminal-preset".into())
                                    .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                                        this.show_choices(
                                            TerminalPalette::ALL.map(Choice::Terminal),
                                            e.position(),
                                            window,
                                            cx,
                                        )
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(colors.muted))
                            .child("Applies to every terminal tab."),
                    )
                    .children(
                        settings
                            .error
                            .iter()
                            .map(|e| div().text_color(rgb(colors.danger)).child(e.clone())),
                    )
                    .child(
                        div()
                            .mt_1()
                            .rounded_md()
                            .overflow_hidden()
                            .border_1()
                            .border_color(rgb(colors.border))
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .bg(rgb(colors.surface))
                                    .text_xs()
                                    .child("Live preview"),
                            )
                            .child(
                                div()
                                    .id("settings-preview")
                                    .debug_selector(|| "settings-preview".into())
                                    .h(px(184.))
                                    .child(settings.preview.clone()),
                            ),
                    ),
            )
    }
}
