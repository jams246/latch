use crate::ui::{Latch, theme::current};
use gpui::{prelude::*, *};

impl Latch {
    pub(super) fn settings_about_page(&self, _: &mut Context<Self>) -> Div {
        let Some(settings) = &self.settings else {
            return div();
        };
        div().size_full().child(
            div()
                .id("settings-about-page")
                .debug_selector(|| "settings-about-page".into())
                .size_full()
                .min_h_0()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .overflow_y_scroll()
                .child(div().text_2xl().child("About"))
                .child(
                    div()
                        .debug_selector(|| "settings-about-version".into())
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(concat!("Latch ", env!("CARGO_PKG_VERSION"))),
                )
                .children(
                    settings
                        .error
                        .iter()
                        .map(|error| div().text_color(rgb(current().danger)).child(error.clone())),
                ),
        )
    }
}
