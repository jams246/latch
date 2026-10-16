use super::{
    Files,
    icons::generic_icon,
    values::{cell, display_size, modified},
};
use crate::ui::files::pane::{Side, entry_type, file_size};
use crate::ui::theme::CARBON;
use gpui::{prelude::*, *};
use latch::sftp::Entry;
use std::sync::Arc;

impl Files {
    pub(super) fn file_row(
        &self,
        side: Side,
        index: usize,
        entry: &Entry,
        icon: Option<Arc<RenderImage>>,
        ordered: Arc<[usize]>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.selections[usize::from(side == Side::Remote)]
            .selected
            .contains(&index);
        let navigable = side == Side::Remote || entry.is_dir || entry.is_link;
        let icon = icon.map_or_else(
            || generic_icon(entry.is_dir, entry.is_link).into_any_element(),
            |icon| img(icon).size_4().flex_shrink_0().into_any_element(),
        );
        row(("entry", index))
            .debug_selector(move || format!("{}-entry-{index}", side.id()))
            .focusable()
            .tab_index(0)
            .when(selected, |row| {
                row.bg(rgb(CARBON.selected))
                    .border_color(rgb(CARBON.accent))
            })
            .hover(move |row| {
                row.bg(rgb(if selected {
                    CARBON.selected
                } else {
                    CARBON.hover
                }))
            })
            .focus(move |row| {
                row.bg(rgb(if selected {
                    CARBON.selected
                } else {
                    CARBON.hover
                }))
                .border_color(rgb(CARBON.accent))
            })
            .when(navigable, |row| row.cursor_pointer())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.selections[usize::from(side == Side::Remote)].click(
                        index,
                        &ordered,
                        event.modifiers,
                    );
                    if event.click_count == 2 && !event.modifiers.control && !event.modifiers.shift
                    {
                        this.activate(side, index, cx);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.selections[usize::from(side == Side::Remote)].context(index);
                    this.open_context_menu(side, Some(index), event.position, window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" {
                    this.activate(side, index, cx);
                }
            }))
            .child(name(entry.name.clone(), icon))
            .child(cell(modified(entry.modified), 130.))
            .child(cell(
                file_size(entry).map_or_else(|| "--".into(), display_size),
                64.,
            ))
            .child(cell(entry_type(entry).to_string(), 60.))
    }

    pub(super) fn parent_row(&self, side: Side, cx: &mut Context<Self>) -> Stateful<Div> {
        row("parent")
            .debug_selector(move || format!("{}-parent", side.id()))
            .cursor_pointer()
            .focusable()
            .tab_index(0)
            .hover(move |row| row.bg(rgb(CARBON.hover)))
            .focus(move |row| row.bg(rgb(CARBON.hover)).border_color(rgb(CARBON.accent)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.selections[usize::from(side == Side::Remote)].clear();
                    if event.click_count == 2 && !event.modifiers.control && !event.modifiers.shift
                    {
                        this.navigate_parent(side, cx);
                    }
                    cx.stop_propagation();
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
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" {
                    this.navigate_parent(side, cx);
                }
            }))
            .child(name(
                "..".into(),
                generic_icon(true, false).into_any_element(),
            ))
            .child(cell("--".into(), 130.))
            .child(cell("--".into(), 64.))
            .child(cell("Folder".into(), 60.))
    }
}

fn row(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .w_full()
        .h_9()
        .flex_shrink_0()
        .text_sm()
        .border_b_1()
        .border_color(rgb(CARBON.border))
        .text_color(rgb(CARBON.text))
}

fn name(value: String, icon: AnyElement) -> Div {
    div()
        .flex()
        .items_center()
        .flex_1()
        .min_w(px(120.))
        .px_2()
        .gap_2()
        .child(icon)
        .child(div().flex_1().min_w_0().truncate().child(value))
}
