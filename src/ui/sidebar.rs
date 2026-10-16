use super::{Latch, button, picker::filter_connections};
use gpui::{prelude::*, *};
use latch::storage::SortMode;

impl Latch {
    pub(super) fn connections_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let colors = super::theme::CARBON;
        let sort = self.store.sort_mode();
        let search = self.connection_search.read(cx).value.to_string();
        let connections = filter_connections(self.store.sorted(), &search);
        let no_matches = connections.is_empty() && !self.store.connections().is_empty();
        let mut rows = Vec::new();
        if sort == SortMode::FrequentAndName {
            rows.push(
                div()
                    .text_xs()
                    .text_color(rgb(colors.accent))
                    .child("TOP 3 FREQUENTLY USED")
                    .into_any_element(),
            );
            rows.extend(
                filter_connections(self.store.frequent(), &search)
                    .iter()
                    .map(|c| self.row(c, "frequent", cx)),
            );
            rows.push(
                div()
                    .mt_3()
                    .text_xs()
                    .text_color(rgb(colors.accent))
                    .child("ALL CONNECTIONS")
                    .into_any_element(),
            );
        }
        rows.extend(connections.iter().map(|c| self.row(c, "all", cx)));
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .gap_3()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_lg().child("Connections"))
                    .child(
                        button("add", "+ Add").on_click(
                            cx.listener(|this, _, window, cx| this.edit(None, window, cx)),
                        ),
                    ),
            )
            .child(
                div()
                    .id("connection-search")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_xs().child("Search connections"))
                    .child(self.connection_search.clone()),
            )
            .child(
                div().flex().gap_1().text_xs().children(
                    [
                        (SortMode::Name, "Name"),
                        (SortMode::MostUsed, "Most used"),
                        (SortMode::FrequentAndName, "Frequent + Name"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (mode, label))| {
                        button(("sort", index), label)
                            .px_2()
                            .bg(rgb(if sort == mode {
                                colors.selected
                            } else {
                                colors.control
                            }))
                            .when(sort == mode, |button| {
                                button.border_color(rgb(colors.accent))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.data_busy || this.shutting_down {
                                    return;
                                }
                                this.save_preference(
                                    "sorting",
                                    move |store| store.set_sort_mode(mode),
                                    cx,
                                );
                                cx.notify();
                            }))
                    }),
                ),
            )
            .child(
                div()
                    .id("connections")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows),
            )
            .children(self.store.connections().is_empty().then(|| {
                div()
                    .text_color(rgb(colors.muted))
                    .child("No saved connections")
            }))
            .children(no_matches.then(|| {
                div()
                    .text_color(rgb(colors.muted))
                    .child("No matching connections")
            }))
            .child(
                button("settings-open", "Settings")
                    .debug_selector(|| "settings-open".into())
                    .flex_shrink_0()
                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
            )
    }
}
