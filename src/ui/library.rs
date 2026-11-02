use super::{
    Latch,
    choices::{Choice, SORTS},
    controls::{icon, quiet},
    picker::filter_connections,
    theme::current,
};
use gpui::{prelude::*, *};
use latch::storage::SortMode;

mod scrollbar;
#[cfg(test)]
mod tests;

impl Latch {
    pub(super) fn connection_library(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let colors = current();
        let sort = self.store(cx).sort_mode();
        let query = &self.connection_search.read(cx).value;
        let matches = filter_connections(self.store(cx).sorted(), query);
        let count = matches.len();
        let mut rows = Vec::new();
        let mut letters = Vec::new();
        if sort == SortMode::FrequentAndName {
            let top = filter_connections(self.store(cx).frequent(), query);
            if !top.is_empty() {
                rows.push(section("Frequently used"));
                letters.push(None);
                for c in top {
                    rows.push(self.row(c, "frequent", None, cx));
                    letters.push(None);
                }
                rows.push(section("All connections"));
                letters.push(None);
            }
        }
        let mut previous = None;
        for c in matches {
            let letter = (sort != SortMode::MostUsed)
                .then(|| c.name.trim().chars().next())
                .flatten()
                .map(|c| c.to_uppercase().to_string());
            let initial = letter.as_ref().filter(|_| letter != previous).cloned();
            letters.push(letter.clone());
            previous = letter;
            rows.push(self.row(c, "all", initial, cx));
        }
        if rows.is_empty() {
            rows.push(
                div()
                    .p_3()
                    .text_color(rgb(colors.muted))
                    .child(if self.store(cx).connections().is_empty() {
                        "No saved connections"
                    } else {
                        "No matching connections"
                    })
                    .into_any_element(),
            )
        }
        let (top, offset) = self.connection_scroll.logical_scroll_top();
        let sticky = letters.get(top).and_then(Option::as_ref).map(|letter| {
            let y = if letters.get(top + 1).and_then(Option::as_ref) == Some(letter) {
                px(0.)
            } else {
                offset
            };
            (letter.clone(), y)
        });
        let search = div()
            .flex()
            .items_center()
            .gap_1()
            .child(icon("\u{e721}"))
            .child(
                div()
                    .w(px(if window.viewport_size().width < px(800.) {
                        115.
                    } else {
                        166.
                    }))
                    .relative()
                    .child(self.connection_search.clone())
                    .when(query.is_empty(), |d| {
                        d.child(
                            div()
                                .absolute()
                                .left(px(9.))
                                .top(px(7.))
                                .text_sm()
                                .text_color(rgb(colors.muted))
                                .child("Find a host"),
                        )
                    }),
            );
        let sort_button = quiet("sort-menu", Choice::Sort(sort).label())
            .debug_selector(|| "sort-menu".into())
            .flex()
            .items_center()
            .gap_1()
            .child("▾")
            .on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                this.show_choices(SORTS.map(Choice::Sort), e.position(), window, cx)
            }));
        let heading = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Connections"),
            )
            .child(
                div()
                    .rounded_sm()
                    .px_1()
                    .bg(rgb(colors.control))
                    .text_xs()
                    .child(count.to_string()),
            );
        let tools = div()
            .flex()
            .items_center()
            .gap_2()
            .child(search)
            .child(sort_button)
            .child(
                quiet("add", "+ Add")
                    .debug_selector(|| "add".into())
                    .on_click(cx.listener(|this, _, window, cx| this.edit(None, window, cx))),
            );
        div()
            .debug_selector(|| "connection-library".into())
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(heading)
                    .child(tools),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .pl(px(58.))
                    .pr(px(26.))
                    .h(px(28.))
                    .flex_shrink_0()
                    .bg(rgb(colors.surface))
                    .text_xs()
                    .text_color(rgb(colors.muted))
                    .child(
                        div()
                            .flex_1()
                            .px_1()
                            .flex()
                            .child(div().flex_1().child("CONNECTION"))
                            .child(div().w(relative(0.22)).min_w(px(44.)).child("USER")),
                    )
                    .child(div().w(px(72.)).text_center().child("FILES"))
                    .child(div().w(px(32.))),
            )
            .child(
                div()
                    .relative()
                    .overflow_hidden()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("connections")
                            .size_full()
                            .flex()
                            .flex_col()
                            .pl(px(16.))
                            .pr(px(26.))
                            .overflow_y_scroll()
                            .track_scroll(&self.connection_scroll)
                            .children(rows),
                    )
                    .children(sticky.map(|(letter, y)| {
                        div()
                            .absolute()
                            .top(y)
                            .left(px(16.))
                            .w(px(36.))
                            .h(px(39.))
                            .flex()
                            .items_center()
                            .text_lg()
                            .bg(rgb(colors.background))
                            .text_color(rgb(colors.accent))
                            .child(letter)
                    }))
                    .child(self.connection_scrollbar(cx)),
            )
    }
}
fn section(label: &'static str) -> AnyElement {
    div()
        .h(px(30.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(current().muted))
        .child(label)
        .into_any_element()
}
