#[cfg(test)]
mod action_tests;
#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod tests;
mod view;

use crate::ui::{Latch, SessionKind, TabContent};
use gpui::*;

pub(in crate::ui) struct TabMenu {
    pub(in crate::ui) tab_id: String,
    position: Point<Pixels>,
    focus: FocusHandle,
    previous: Option<FocusHandle>,
    duplicate: FocusHandle,
    close: FocusHandle,
}

impl Latch {
    pub(in crate::ui) fn open_tab_menu(
        &mut self,
        id: &str,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shutting_down || self.data_busy || self.settings.is_some() {
            return;
        }
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return;
        };
        let can_duplicate = matches!(tab.content, TabContent::Terminal(_)) && !tab.closing;
        let menu = TabMenu {
            tab_id: id.to_owned(),
            position,
            focus: cx.focus_handle().tab_stop(false),
            previous: window.focused(cx),
            duplicate: cx.focus_handle().tab_stop(true),
            close: cx.focus_handle().tab_stop(true),
        };
        window.focus(if can_duplicate {
            &menu.duplicate
        } else {
            &menu.close
        });
        self.tab_drag = super::TabDrag::default();
        self.tab_menu = Some(menu);
        cx.notify();
    }

    pub(in crate::ui) fn dismiss_tab_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.tab_menu.take() {
            if let Some(focus) = menu.previous {
                window.focus(&focus);
            } else {
                window.blur();
            }
            cx.notify();
        }
    }

    fn choose_tab_action(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.tab_menu.as_ref().map(|menu| menu.tab_id.clone()) else {
            return;
        };
        self.dismiss_tab_menu(window, cx);
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        if duplicate {
            self.duplicate_tab(index, window, cx);
        } else {
            self.close_tab(index, window, cx);
        }
    }

    fn duplicate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        if tab.closing || !matches!(tab.content, TabContent::Terminal(_)) {
            return;
        }
        let connection = tab.connection.clone();
        let count = self.tabs.len();
        self.connect(connection, SessionKind::Terminal, window, cx);
        if self.tabs.len() == count + 1 {
            self.tabs[index + 1..].rotate_right(1);
            self.active = Some(index + 1);
            cx.notify();
        }
    }

    fn tab_menu_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = &self.tab_menu else { return };
        match event.keystroke.key.as_str() {
            "escape" => self.dismiss_tab_menu(window, cx),
            "up" | "down" | "tab" => {
                let enabled = self.tabs.iter().any(|tab| {
                    tab.id == menu.tab_id
                        && matches!(tab.content, TabContent::Terminal(_))
                        && !tab.closing
                }) && !self.data_busy
                    && !self.shutting_down;
                window.focus(if enabled && menu.close.is_focused(window) {
                    &menu.duplicate
                } else {
                    &menu.close
                });
            }
            _ => (),
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    fn tab_menu_key_up(&mut self, event: &KeyUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(event.keystroke.key.as_str(), "enter" | "space")
            && let Some(menu) = &self.tab_menu
        {
            self.choose_tab_action(menu.duplicate.is_focused(window), window, cx);
        }
        window.prevent_default();
        cx.stop_propagation();
    }
}
