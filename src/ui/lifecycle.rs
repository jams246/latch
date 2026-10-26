mod window_close;

use gpui::*;
use latch::{sftp::Transfer, ssh};

use super::{Latch, Tab, TabContent, tabs, transport::Transport};

pub(super) struct Completion {
    id: String,
    result: Result<(), String>,
}

fn completed_state(state: ssh::State) -> ssh::State {
    match state {
        ssh::State::Failed => ssh::State::Failed,
        _ => ssh::State::Disconnected,
    }
}

fn begin_disconnect(state: &mut ssh::State, closing: &mut bool, has_transport: bool) -> bool {
    if *closing {
        return false;
    }
    if !has_transport {
        *state = completed_state(*state);
        return false;
    }
    *closing = true;
    true
}

impl Tab {
    pub(super) fn visible_focus(&self, cx: &App) -> FocusHandle {
        match &self.content {
            TabContent::Terminal(terminal) => terminal.focus_handle(cx),
            TabContent::Sftp {
                files: Some(files), ..
            } => files.read(cx).path.focus_handle(cx),
            TabContent::Sftp { focus, .. } => focus.clone(),
        }
    }

    fn detach(&mut self, cx: &mut App) -> (Option<Transport>, Option<Transfer>) {
        self.closing = true;
        self.password_attempt = None;
        self.host_dialog.take();
        if let Some(dialog) = self.dialog.take() {
            let _ = dialog.update(cx, |dialog, window, _| dialog.close(window));
        }
        let transfer = match &mut self.content {
            TabContent::Sftp { files, .. } => files
                .take()
                .and_then(|files| files.update(cx, |files, cx| files.prepare_close(cx))),
            TabContent::Terminal(_) => None,
        };
        let transport = self.transport.take();
        if let Some(transport) = &transport {
            transport.stop_input();
        }
        (transport, transfer)
    }
}

impl Latch {
    fn schedule_cleanup(
        &mut self,
        id: String,
        transport: Option<Transport>,
        transfer: Option<Transfer>,
    ) {
        let Some(transport) = transport else {
            return;
        };
        self.pending_cleanup += 1;
        let completion = self.cleanup_sender.clone();
        self.runtime.spawn(async move {
            let result = transport.shutdown(transfer).await;
            let _ = completion.send(Completion { id, result });
        });
    }

    pub(super) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self
            .tab_menu
            .as_ref()
            .is_some_and(|menu| menu.tab_id == self.tabs[index].id)
        {
            self.dismiss_tab_menu(window, cx);
        }
        let mut tab = self.tabs.remove(index);
        let (transport, transfer) = tab.detach(cx);
        self.schedule_cleanup(tab.id.clone(), transport, transfer);
        self.active = tabs::active_after_close(self.active, index, self.tabs.len());
        if self.settings.is_none() && self.tab_menu.is_none() {
            let focus = self
                .active
                .and_then(|active| self.tabs.get(active))
                .map(|tab| tab.visible_focus(cx))
                .unwrap_or_else(|| {
                    if self.sidebar_collapsed {
                        self.sidebar_toggle_focus.clone()
                    } else {
                        self.connection_search.read(cx).focus.clone()
                    }
                });
            window.focus(&focus);
        }
        cx.notify();
    }

    pub(super) fn disconnect(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get_mut(index) else {
            return;
        };
        if !begin_disconnect(&mut tab.state, &mut tab.closing, tab.transport.is_some()) {
            cx.notify();
            return;
        }
        let id = tab.id.clone();
        let (transport, transfer) = tab.detach(cx);
        self.schedule_cleanup(id, transport, transfer);
        cx.notify();
    }

    /// Defers main-window closure until transfer cleanup and transport shutdown finish.
    pub(crate) fn request_shutdown(&mut self, cx: &mut Context<Self>) -> bool {
        if self.data_busy {
            return false;
        }
        self.cancel_tab_drag(cx);
        self.poll_local_preferences(cx);
        self.tab_menu = None;
        if self.tabs.is_empty() && self.pending_cleanup == 0 && !self.saves_pending() {
            return true;
        }
        if self.shutting_down {
            return false;
        }
        self.shutting_down = true;
        self.error = Some("Saving preferences and closing connections…".into());
        for mut tab in std::mem::take(&mut self.tabs) {
            let (transport, transfer) = tab.detach(cx);
            self.schedule_cleanup(tab.id.clone(), transport, transfer);
        }
        self.active = None;
        cx.notify();
        false
    }

    pub(super) fn poll_cleanup(&mut self, cx: &mut Context<Self>) {
        while let Ok(completion) = self.cleanup_events.try_recv() {
            self.pending_cleanup = self.pending_cleanup.saturating_sub(1);
            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == completion.id) {
                tab.closing = false;
                tab.state = completed_state(tab.state);
                if let Err(error) = &completion.result {
                    tab.error = Some(error.clone());
                }
            }
            if let Err(error) = completion.result {
                self.error = Some(format!("Connection cleanup failed: {error}"));
                self.shutting_down = false;
            }
            cx.notify();
        }
        if self.shutting_down && self.pending_cleanup == 0 && !self.saves_pending() {
            self.finish_window_close(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{begin_disconnect, completed_state, ssh::State};

    #[test]
    fn repeated_disconnect_without_owner_requires_no_completion_and_stays_settled() {
        for expected in [State::Disconnected, State::Failed] {
            let mut state = expected;
            let mut closing = false;
            for _ in 0..3 {
                assert!(!begin_disconnect(&mut state, &mut closing, false));
                assert_eq!(state, expected);
                assert!(!closing);
            }
        }
    }

    #[test]
    fn disconnect_with_owner_schedules_cleanup_once_and_waits_for_completion() {
        let mut state = State::Connected;
        let mut closing = false;
        assert!(begin_disconnect(&mut state, &mut closing, true));
        assert!(closing);
        assert!(!begin_disconnect(&mut state, &mut closing, false));
        assert_eq!(state, State::Connected);
        assert!(closing);
    }

    #[test]
    fn cleanup_should_preserve_failed_connection_state() {
        assert_eq!(completed_state(State::Failed), State::Failed);
    }

    #[test]
    fn cleanup_should_finish_other_states_as_disconnected() {
        for state in [
            State::Connecting,
            State::Authenticating,
            State::Connected,
            State::Disconnected,
        ] {
            assert_eq!(completed_state(state), State::Disconnected);
        }
    }
}
