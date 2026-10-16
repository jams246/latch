use super::{Latch, button, picker::filter_connections};
use gpui::{prelude::*, *};
use latch::storage::SortMode;

impl Render for Latch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reveal_key = self.active.and_then(|index| {
            self.tabs
                .get(index)
                .map(|tab| (tab.id.clone(), index, window.viewport_size().width))
        });
        if self.tab_reveal_key != reveal_key {
            if let Some((_, index, _)) = &reveal_key {
                let index = *index;
                self.tab_scroll.scroll_to_item(index);
                let entity = cx.weak_entity();
                // GPUI's scroll handle receives the resized viewport during prepaint.
                window.on_next_frame(move |_, cx| {
                    let _ = entity.update(cx, |this, cx| {
                        if let Some(index) = this.active {
                            this.tab_scroll.scroll_to_item(index);
                            cx.notify();
                        }
                    });
                });
            }
            self.tab_reveal_key = reveal_key;
        }
        let sort = self.store.sort_mode();
        let search = self.connection_search.read(cx).value.to_string();
        let connections = filter_connections(self.store.sorted(), &search);
        let no_matches = connections.is_empty() && !self.store.connections().is_empty();
        let mut rows = Vec::new();
        if sort == SortMode::FrequentAndName {
            rows.push(
                div()
                    .text_xs()
                    .text_color(rgb(0x75d9c0))
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
                    .text_color(rgb(0x75d9c0))
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
            .bg(rgb(0x111b25))
            .text_color(rgb(0xe4edf5))
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
                    .bg(rgb(0x182531))
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
                                    .bg(rgb(if sort == mode { 0x276453 } else { 0x263c4d }))
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
                            .text_color(rgb(0xa8c1d3))
                            .child("No saved connections")
                    }))
                    .children(no_matches.then(|| {
                        div()
                            .text_color(rgb(0xa8c1d3))
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
                    .child(
                        div()
                            .id("tabs")
                            .flex()
                            .gap_1()
                            .p_2()
                            .overflow_x_scroll()
                            .track_scroll(&self.tab_scroll)
                            .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                                div()
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .rounded_md()
                                    .bg(rgb(if self.active == Some(index) {
                                        0x315267
                                    } else {
                                        0x263c4d
                                    }))
                                    .child(
                                        button(
                                            SharedString::from(format!("close-{}", tab.id)),
                                            "×",
                                        )
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .size(px(32.))
                                        .p_0()
                                        .bg(rgba(0))
                                        .text_size(px(22.))
                                        .text_color(rgb(0xffa69b))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.close_tab(index, window, cx)
                                            }),
                                        ),
                                    )
                                    .child(
                                        button(
                                            SharedString::from(format!("tab-{}", tab.id)),
                                            tab.name.clone(),
                                        )
                                        .px(px(10.))
                                        .bg(rgba(0))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.active = Some(index);
                                                if let Some(tab) = this.tabs.get(index) {
                                                    window.focus(&tab.visible_focus(cx));
                                                }
                                                cx.notify();
                                            }),
                                        ),
                                    )
                            })),
                    )
                    .children(
                        self.error
                            .clone()
                            .filter(|_| self.editor.is_none())
                            .map(|error| div().p_3().text_color(rgb(0xffa69b)).child(error)),
                    )
                    .child(content),
            )
            .when(self.editor.is_some(), |view| {
                view.child(self.editor_view(cx))
            })
    }
}
