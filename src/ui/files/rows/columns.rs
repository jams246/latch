use super::{Files, TABLE_WIDTH};
use crate::ui::{
    button,
    files::pane::{Column, Side},
    theme::CARBON,
};
use gpui::{prelude::*, *};

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
            .bg(rgb(CARBON.control))
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
                            cell.child(
                                div()
                                    .text_color(rgb(CARBON.accent))
                                    .child(if sort.descending { "▼" } else { "▲" }),
                            )
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            match side {
                                Side::Local => this.local_sort.toggle(column),
                                Side::Remote => this.remote_sort.toggle(column),
                            }
                            this.rebuild_order(side);
                            cx.notify();
                        }))
                }),
            )
    }
}
