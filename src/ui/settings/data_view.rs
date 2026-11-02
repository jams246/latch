use super::*;
use crate::ui::{button, theme::current};
use gpui::prelude::*;

impl Latch {
    pub(super) fn settings_data_page(&self, cx: &mut Context<Self>) -> Div {
        let Some(settings) = &self.settings else {
            return div();
        };
        div().size_full().child(div()
            .id("settings-data-page")
            .debug_selector(|| "settings-data-page".into())
            .size_full()
            .min_h_0()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .overflow_y_scroll()
            .child(div().text_2xl().child("Data"))
            .child(div().font_weight(FontWeight::SEMIBOLD).child("Portable backup"))
            .child(div().text_color(rgb(current().muted)).child(
                "Exports are unencrypted JSON. They contain connection details, settings and key-file paths, but no passwords or private keys. Keep them somewhere safe.",
            ))
            .child(
                button("settings-export", "Export data")
                    .debug_selector(|| "settings-export".into())
                    .when(self.data_busy || self.saves_pending(), |button| button.opacity(0.5))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.export_data(window, cx);
                    })),
            )
            .child(div().mt_2().font_weight(FontWeight::SEMIBOLD).child("Restore from backup"))
            .child(div().text_color(rgb(current().muted)).child("Import replaces your saved connections and settings."))
            .child(
                button("settings-import", "Import data")
                    .debug_selector(|| "settings-import".into())
                    .when(!self.can_import(cx), |button| button.opacity(0.5))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.import_data(window, cx);
                    })),
            )
            .children((!self.can_import(cx) && !self.data_busy).then(|| {
                div().text_color(rgb(current().muted)).child(
                    "Close other Latch windows and all terminal and SFTP tabs, then wait for saves and connections to finish before importing.",
                )
            }))
            .children(self.data_busy.then(|| div().child("Working...")))
            .children(settings.message.iter().map(|message| {
                div().text_color(rgb(current().accent)).child(message.clone())
            }))
            .children(settings.error.iter().map(|error| {
                div().text_color(rgb(current().danger)).child(error.clone())
            })))
    }
}
