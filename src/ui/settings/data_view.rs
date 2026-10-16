use super::*;
use crate::ui::{button, theme::CARBON};
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
            .track_focus(&settings.focus)
            .capture_key_down(cx.listener(Self::settings_key_down))
            .child(div().text_2xl().child("Data"))
            .child(div().text_color(rgb(CARBON.muted)).child(
                "Export a portable backup or replace your saved data with an existing export.",
            ))
            .child(div().text_color(rgb(CARBON.muted)).child(
                "Exports are unencrypted JSON. They contain connection details, settings and key-file paths, but no passwords or private keys. Keep them somewhere safe.",
            ))
            .children(self.store.warnings().iter().map(|warning| {
                div().text_color(rgb(CARBON.danger)).child(warning.clone())
            }))
            .child(
                button("settings-export", "Export data")
                    .debug_selector(|| "settings-export".into())
                    .when(self.data_busy, |button| button.opacity(0.5))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.export_data(window, cx);
                    })),
            )
            .child(
                button("settings-import", "Import data")
                    .debug_selector(|| "settings-import".into())
                    .when(!self.can_import(), |button| button.opacity(0.5))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.import_data(window, cx);
                    })),
            )
            .children((!self.can_import() && !self.data_busy).then(|| {
                div().text_color(rgb(CARBON.muted)).child(
                    "Close all terminal and SFTP tabs, then wait for them to finish closing before importing.",
                )
            }))
            .children(self.data_busy.then(|| div().child("Working...")))
            .children(settings.message.iter().map(|message| {
                div().text_color(rgb(CARBON.accent)).child(message.clone())
            }))
            .children(settings.error.iter().map(|error| {
                div().text_color(rgb(CARBON.danger)).child(error.clone())
            })))
    }
}
