use crate::ui::{
    Latch, TabContent,
    controls::{Hint, icon, quiet},
    theme::current,
};
use gpui::{prelude::*, *};
impl Latch {
    pub(super) fn tab_header(
        &self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let tab = &self.tabs[index];
        let colors = current();
        let name = tab.name.clone();
        let address = tab.destination.clone();
        let tab_id = tab.id.clone();
        let drag_id = tab.id.clone();
        let group = SharedString::from(format!("session-{}", tab.id));
        let focused = tab.header_focus.contains_focused(window, cx) || super::native::touch_only();
        let active = self.settings.is_none() && self.active == Some(index);
        div()
            .group(group.clone())
            .track_focus(&tab.header_focus)
            .id(SharedString::from(format!("tab-header-{}", tab.id)))
            .capture_any_mouse_down(
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if event.button == MouseButton::Right {
                        this.open_tab_menu(&tab_id, event.position, window, cx);
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                }),
            )
            .flex()
            .flex_shrink_0()
            .opacity(if self.tab_drag.dragging(&tab.id) {
                0.45
            } else {
                1.
            })
            .items_center()
            .h(px(44.))
            .px_1()
            .border_b_2()
            .border_color(if active {
                rgb(colors.accent)
            } else {
                rgb(colors.panel)
            })
            .bg(rgb(if active {
                colors.background
            } else {
                colors.panel
            }))
            .child(
                quiet(SharedString::from(format!("close-{}", tab.id)), "")
                    .debug_selector(|| format!("tab-close-{index}"))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .relative()
                    .p_0()
                    .text_color(rgb(colors.muted))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(if focused { 0. } else { 1. })
                            .group_hover(group.clone(), |s| s.opacity(0.))
                            .child(if matches!(tab.content, TabContent::Terminal(_)) {
                                div()
                                    .font_family("Cascadia Mono")
                                    .text_sm()
                                    .child(">_")
                                    .into_any_element()
                            } else {
                                icon("\u{ed41}").into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .opacity(if focused { 1. } else { 0. })
                            .group_hover(group, |s| s.opacity(1.))
                            .child("×"),
                    )
                    .on_click(cx.listener(move |this, event, window, cx| {
                        if this.data_busy || this.shutting_down {
                            return;
                        }
                        if this.tab_drag.did_drag && matches!(event, ClickEvent::Mouse(_)) {
                            return;
                        }
                        this.close_tab(index, window, cx);
                    })),
            )
            .child(
                quiet(
                    SharedString::from(format!("tab-{}", tab.id)),
                    match &tab.content {
                        TabContent::Terminal(_) => format!("{} · Terminal", tab.name),
                        TabContent::Sftp { .. } => tab.name.clone(),
                    },
                )
                .debug_selector(|| format!("tab-name-{index}"))
                .max_w(px(164.))
                .min_w_0()
                .truncate()
                .tooltip(move |_, cx| cx.new(|_| Hint(name.clone(), address.clone())).into())
                .text_sm()
                .px(px(10.))
                .bg(rgba(0))
                .border_color(rgba(0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, window, cx| {
                        if this.active == Some(index) {
                            this.begin_tab_move(drag_id.clone(), window, cx);
                        }
                    }),
                )
                .on_click(cx.listener(move |this, event, window, cx| {
                    if this.tab_drag.did_drag && matches!(event, ClickEvent::Mouse(_)) {
                        return;
                    }
                    if this.data_busy || this.shutting_down {
                        return;
                    }
                    this.settings = None;
                    this.active = Some(index);
                    if let Some(tab) = this.tabs.get(index) {
                        window.focus(&tab.visible_focus(cx));
                    }
                    cx.notify();
                })),
            )
    }
}
