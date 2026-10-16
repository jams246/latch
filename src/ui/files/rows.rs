use super::{
    Files,
    pane::{Column, Side, entry_type, file_size},
};
use crate::ui::button;
use gpui::{prelude::*, *};
use latch::sftp::Entry;
use std::sync::Arc;

const TABLE_WIDTH: f32 = 374.;

impl Files {
    pub(in crate::ui) fn columns(&self, side: Side, cx: &mut Context<Self>) -> Div {
        let sort = match side {
            Side::Local => self.local_sort,
            Side::Remote => self.remote_sort,
        };
        div()
            .flex()
            .min_w(px(TABLE_WIDTH))
            .w_full()
            .flex_shrink_0()
            .bg(rgb(0x263c4d))
            .children(
                [
                    (Column::Name, "Name"),
                    (Column::Modified, "Date Modified"),
                    (Column::Size, "Size"),
                    (Column::Type, "Type"),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (column, label))| {
                    button(("column", index), label)
                        .debug_selector(move || format!("{}-column-{index}", side.id()))
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py_2()
                        .rounded_none()
                        .text_xs()
                        .when(column == Column::Name, |cell| cell.flex_1().min_w(px(120.)))
                        .when(column == Column::Modified, |cell| {
                            cell.w(px(130.)).flex_shrink_0()
                        })
                        .when(column == Column::Size, |cell| {
                            cell.w(px(64.)).flex_shrink_0()
                        })
                        .when(column == Column::Type, |cell| {
                            cell.w(px(60.)).flex_shrink_0()
                        })
                        .when(sort.column == column, |cell| {
                            cell.child(div().text_color(rgb(0x9ce6d6)).child(if sort.descending {
                                "▼"
                            } else {
                                "▲"
                            }))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            match side {
                                Side::Local => this.local_sort.toggle(column),
                                Side::Remote => this.remote_sort.toggle(column),
                            }
                            cx.notify();
                        }))
                }),
            )
    }

    pub(in crate::ui) fn file_rows(&self, side: Side, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut rows: Vec<_> = match side {
            Side::Local => self.local.as_ref().map_or_else(Vec::new, |directory| {
                directory
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| (index, &entry.metadata, entry.icon.as_ref()))
                    .collect()
            }),
            Side::Remote => self.directory.as_ref().map_or_else(Vec::new, |directory| {
                directory
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| (index, entry, None))
                    .collect()
            }),
        };
        let sort = match side {
            Side::Local => self.local_sort,
            Side::Remote => self.remote_sort,
        };
        rows.retain(|(_, entry, _)| !matches!(entry.name.as_str(), "." | ".."));
        rows.sort_by(|left, right| sort.compare(left.1, right.1));
        div()
            .id("entries")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w(px(TABLE_WIDTH))
            .w_full()
            .overflow_y_scroll()
            .bg(rgb(0x192a37))
            .children(
                rows.into_iter().map(|(index, entry, icon)| {
                    self.file_row(side, index, entry, icon.cloned(), cx)
                }),
            )
    }

    fn file_row(
        &self,
        side: Side,
        index: usize,
        entry: &Entry,
        icon: Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let navigable = side == Side::Remote || entry.is_dir || entry.is_link;
        div()
            .id(("entry", index))
            .debug_selector(move || format!("{}-entry-{index}", side.id()))
            .flex()
            .items_center()
            .w_full()
            .h_9()
            .flex_shrink_0()
            .text_sm()
            .border_b_1()
            .border_color(rgb(0x233847))
            .hover(|row| row.bg(rgb(0x304d62)))
            .when(navigable, |row| {
                row.cursor_pointer()
                    .focusable()
                    .tab_index(0)
                    .focus(|row| row.bg(rgb(0x304d62)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            if event.click_count == 2 {
                                this.activate(side, index, cx);
                            }
                        }),
                    )
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            this.activate(side, index, cx);
                        }
                    }))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w(px(120.))
                    .px_2()
                    .gap_2()
                    .when_some(icon, |cell, icon| {
                        cell.child(img(icon).size_4().flex_shrink_0())
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(entry.name.clone()),
                    ),
            )
            .child(cell(modified(entry.modified), 130.))
            .child(cell(
                file_size(entry).map_or_else(|| "--".into(), display_size),
                64.,
            ))
            .child(cell(entry_type(entry).to_string(), 60.))
    }
}

fn cell(value: String, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .px_2()
        .text_xs()
        .truncate()
        .child(value)
}

fn modified(seconds: Option<u64>) -> String {
    seconds
        .and_then(|seconds| i64::try_from(seconds).ok())
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .map_or_else(
            || "--".into(),
            |date| date.format("%Y-%m-%d %H:%M").to_string(),
        )
}

fn display_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    for unit in ["KB", "MB", "GB", "TB", "PB", "EB"] {
        value /= 1024.;
        if value < 1024. || unit == "EB" {
            return if value < 10. {
                format!("{value:.1} {unit}")
            } else {
                format!("{value:.0} {unit}")
            };
        }
    }
    format!("{bytes} B")
}

#[cfg(test)]
mod tests {
    use super::{display_size, modified};

    #[test]
    fn sizes_stay_readable_in_compact_columns() {
        assert_eq!(display_size(1023), "1023 B");
        assert_eq!(display_size(1024), "1.0 KB");
        assert_eq!(display_size(1_048_576), "1.0 MB");
        assert_eq!(display_size(u64::MAX), "16 EB");
    }

    #[test]
    fn dates_use_utc_and_unavailable_dates_have_placeholder() {
        assert_eq!(modified(Some(0)), "1970-01-01 00:00");
        assert_eq!(modified(None), "--");
        assert_eq!(modified(Some(u64::MAX)), "--");
    }
}
