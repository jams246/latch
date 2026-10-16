use super::{Latch, button, picker::filter_connections};
use gpui::{prelude::*, *};
use latch::storage::SortMode;

impl Render for Latch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = super::theme::CARBON;
        let tabs = self.tab_bar(window, cx);
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
        let content = self.workspace(cx);
        div()
            .id("latch")
            .relative()
            .size_full()
            .flex()
            .bg(rgb(colors.background))
            .text_color(rgb(colors.text))
            .text_size(px(14.))
            .font_family("Segoe UI")
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(310.))
                    .flex_shrink_0()
                    .p_3()
                    .gap_3()
                    .bg(rgb(colors.panel))
                    .border_r_1()
                    .border_color(rgb(colors.border))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(div().text_lg().child("Connections"))
                            .child(button("add", "+ Add").on_click(
                                cx.listener(|this, _, window, cx| this.edit(None, window, cx)),
                            )),
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
                                        if let Err(error) = this.store.set_sort_mode(mode) {
                                            this.error =
                                                Some(format!("Cannot save sorting: {error}"));
                                        }
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
                    .children((self.store.connections().is_empty()).then(|| {
                        div()
                            .text_color(rgb(colors.muted))
                            .child("No saved connections")
                    }))
                    .children(no_matches.then(|| {
                        div()
                            .text_color(rgb(colors.muted))
                            .child("No matching connections")
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
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
            .when(self.editor.is_some(), |view| {
                view.child(self.editor_view(cx))
            })
            .when(self.connection_delete.is_some(), |view| {
                view.child(self.connection_delete_view(cx))
            })
    }
}
