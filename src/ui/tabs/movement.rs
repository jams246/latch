use crate::ui::{Latch, TabContent};
use gpui::*;
use latch::ssh::State;

impl Latch {
    pub(in crate::ui) fn accepts_tab(&self) -> bool {
        !self.shutting_down
            && !self.data_busy
            && self.settings.is_none()
            && self.editor.is_none()
            && self.connection_delete.is_none()
            && self.tab_menu.is_none()
    }

    pub(in crate::ui) fn movable_tab(&self, id: &str) -> bool {
        self.accepts_tab()
            && self
                .active
                .and_then(|index| self.tabs.get(index))
                .is_some_and(|tab| tab.id == id)
            && self.tabs.iter().any(|tab| {
                tab.id == id
                    && !tab.closing
                    && tab.dialog.is_none()
                    && tab.host_dialog.is_none()
                    && tab.reconnect.is_none()
                    && matches!(
                        tab.state,
                        State::Connected | State::Disconnected | State::Failed
                    )
            })
    }

    pub(in crate::ui) fn move_tab_to(
        &mut self,
        id: &str,
        target: WindowHandle<Latch>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if target.window_id() == self.owner.window_id() || !self.movable_tab(id) {
            return false;
        }
        // Validate destination before removing ownership from the source.
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        self.poll_local_preferences(cx);
        let result = target
            .update(cx, |destination, target_window, cx| {
                if !destination.accepts_tab() || destination.profile != self.profile {
                    return false;
                }

                let tab = self.tabs.remove(index);
                match &tab.content {
                    TabContent::Terminal(terminal) => {
                        terminal.update(cx, |terminal, _| terminal.prepare_window_move())
                    }
                    TabContent::Sftp {
                        files: Some(files), ..
                    } => files.update(cx, |files, _| {
                        files.preference_sender = destination.preference_sender.clone();
                    }),
                    _ => (),
                }
                destination.tabs.push(tab);
                destination.active = Some(destination.tabs.len() - 1);
                destination.tab_reveal_key = None;
                destination.drop_highlight = false;
                target_window
                    .focus(&destination.tabs[destination.active.unwrap_or(0)].visible_focus(cx));
                target_window.activate_window();
                cx.notify();
                true
            })
            .unwrap_or(false);
        if result {
            self.active = super::active_after_close(self.active, index, self.tabs.len());
            self.tab_reveal_key = None;
            if let Some(tab) = self.active.and_then(|index| self.tabs.get(index)) {
                window.focus(&tab.visible_focus(cx));
            } else {
                window.focus(&self.sidebar_toggle_focus);
            }
            cx.notify();
        }
        result
    }
}
