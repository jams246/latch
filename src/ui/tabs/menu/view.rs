use crate::ui::{Latch, TabContent, button, theme::CARBON};
use gpui::{prelude::*, *};

impl Latch {
    pub(in crate::ui) fn tab_menu_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.tab_menu else {
            return div().absolute().into_any_element();
        };
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == menu.tab_id) else {
            return div().absolute().into_any_element();
        };
        let terminal = matches!(tab.content, TabContent::Terminal(_));
        let enabled = !tab.closing && !self.data_busy && !self.shutting_down;
        div()
            .id("tab-menu-overlay")
            .debug_selector(|| "tab-menu-overlay".into())
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.dismiss_tab_menu(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, window, cx| {
                    this.dismiss_tab_menu(window, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                anchored()
                    .position(menu.position)
                    .snap_to_window_with_margin(Edges::all(px(1.)))
                    .child(
                        div()
                            .id("tab-context-menu")
                            .debug_selector(|| "tab-context-menu".into())
                            .track_focus(&menu.focus)
                            .tab_group()
                            .occlude()
                            .w(px(180.))
                            .p_1()
                            .flex()
                            .flex_col()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(CARBON.border))
                            .bg(rgb(CARBON.panel))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                            .capture_key_down(cx.listener(Self::tab_menu_key_down))
                            .capture_key_up(cx.listener(Self::tab_menu_key_up))
                            .children(
                                [
                                    (true, "Duplicate", "tab-menu-duplicate", &menu.duplicate),
                                    (false, "Close", "tab-menu-close", &menu.close),
                                ]
                                .into_iter()
                                .filter(|(duplicate, ..)| !duplicate || terminal)
                                .map(
                                    |(duplicate, label, id, focus)| {
                                        button(id, label)
                                            .debug_selector(move || id.into())
                                            .track_focus(focus)
                                            .rounded_none()
                                            .bg(rgb(CARBON.panel))
                                            .py_2()
                                            .when(duplicate && !enabled, |row| {
                                                row.opacity(0.35).tab_index(-1)
                                            })
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.choose_tab_action(duplicate, window, cx);
                                                cx.stop_propagation();
                                            }))
                                    },
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
