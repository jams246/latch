mod columns;
mod icons;
#[cfg(test)]
mod interaction_tests;
mod row;
pub(super) mod values;

use super::{Files, pane::Side};
use crate::ui::theme::CARBON;
use gpui::{prelude::*, *};

const TABLE_WIDTH: f32 = 374.;

impl Files {
    pub(super) fn rebuild_order(&mut self, side: Side) {
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
        self.ordered[usize::from(side == Side::Remote)] = rows.iter().map(|row| row.0).collect();
    }

    pub(in crate::ui) fn file_rows(&self, side: Side, cx: &mut Context<Self>) -> Stateful<Div> {
        let ordered = &self.ordered[usize::from(side == Side::Remote)];
        div()
            .id("entries")
            .debug_selector(move || format!("{}-entries", side.id()))
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w(px(TABLE_WIDTH))
            .w_full()
            .overflow_y_scroll()
            .bg(rgb(CARBON.surface))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.selections[usize::from(side == Side::Remote)].clear();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.open_context_menu(side, None, event.position, window, cx);
                    cx.stop_propagation();
                }),
            )
            .child(self.parent_row(side, cx))
            .children(ordered.iter().filter_map(|&index| {
                let (entry, icon) = match side {
                    Side::Local => {
                        let entry = self.local.as_ref()?.entries.get(index)?;
                        (&entry.metadata, entry.icon.clone())
                    }
                    Side::Remote => (self.directory.as_ref()?.entries.get(index)?, None),
                };
                Some(self.file_row(side, index, entry, icon, ordered.clone(), cx))
            }))
    }
}
