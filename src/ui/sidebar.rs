use super::{Latch, button, picker::filter_connections};
use gpui::{prelude::*, *};
use latch::storage::SortMode;

#[cfg(test)]
mod tests;

impl Latch {
    pub(super) fn connections_sidebar(&self, cx: &mut Context<Self>) -> Div {
        if self.sidebar_collapsed {
            return div()
                .flex()
                .flex_col()
                .size_full()
                .child(div().flex_1())
                .child(self.sidebar_toggle(cx));
        }
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
            .debug_selector(|| "connections-sidebar".into())
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
                div()
                    .flex()
                    .gap_2()
                    .flex_shrink_0()
                    .child(
                        button("settings-open", "Settings")
                            .debug_selector(|| "settings-open".into())
                            .flex_1()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_settings(window, cx);
                            })),
                    )
                    .child(self.sidebar_toggle(cx)),
            )
    }

    fn sidebar_toggle(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = super::theme::CARBON;
        let label = if self.sidebar_collapsed {
            "Expand sidebar"
        } else {
            "Collapse sidebar"
        };
        button("sidebar-toggle", "")
            .debug_selector(|| "sidebar-toggle".into())
            .track_focus(&self.sidebar_toggle_focus)
            .size(px(32.))
            .flex_shrink_0()
            .p_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .items_center()
                    .w(px(18.))
                    .h(px(16.))
                    .rounded(px(2.))
                    .border_1()
                    .border_color(rgb(colors.text))
                    .child(
                        div()
                            .w(px(5.))
                            .h_full()
                            .border_r_1()
                            .border_color(rgb(colors.text)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_center()
                            .line_height(px(12.))
                            .child(if self.sidebar_collapsed { "›" } else { "‹" }),
                    ),
            )
            .tooltip(move |_, cx| cx.new(|_| SidebarTooltip(label)).into())
            .on_click(cx.listener(|this, _, window, cx| {
                this.sidebar_collapsed = !this.sidebar_collapsed;
                this.tab_reveal_key = None;
                window.focus(&this.sidebar_toggle_focus);
                cx.notify();
            }))
    }
}

struct SidebarTooltip(&'static str);

impl Render for SidebarTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let colors = super::theme::CARBON;
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(colors.control))
            .border_1()
            .border_color(rgb(colors.border))
            .text_color(rgb(colors.text))
            .text_sm()
            .child(self.0)
    }
}
