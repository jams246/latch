mod authentication;

use super::{Latch, TabContent};
use authentication::PasswordUpdate;
use gpui::*;
use latch::ssh;

#[cfg(test)]
mod tests;

impl Latch {
    pub(super) fn poll(&mut self, cx: &mut Context<Self>) {
        self.poll_cleanup(cx);
        self.poll_local_preferences(cx);
        let mut changed = false;
        let mut successful_shells = Vec::new();
        let mut passwords = Vec::new();
        let mut disconnected = Vec::new();
        let active_id = self
            .active
            .and_then(|index| self.tabs.get(index))
            .map(|tab| tab.id.clone());
        for tab in &mut self.tabs {
            if tab.host_dialog.as_ref().is_some_and(|dialog| {
                dialog.reply.is_closed()
                    || !matches!(
                        tab.state,
                        ssh::State::Connecting
                            | ssh::State::Authenticating
                            | ssh::State::Reconnecting
                    )
            }) {
                if let Some(dialog) = tab.host_dialog.take() {
                    dialog.dismiss(self.owner, cx);
                }
                changed = true;
            }
            while let Ok(event) = tab.events.try_recv() {
                changed = true;
                match event {
                    ssh::Event::Status(state) => {
                        tab.state = state;
                        if state == ssh::State::Reconnecting && !tab.closing {
                            tab.error = None;
                            if tab.reconnect.is_none() {
                                tab.reconnect = Some(super::reconnect::ReconnectDialog::new(cx));
                                if let TabContent::Sftp {
                                    files: Some(files), ..
                                } = &tab.content
                                {
                                    files.update(cx, |files, cx| files.connection_lost(cx));
                                }
                            }
                        }
                        if matches!(state, ssh::State::Failed | ssh::State::Disconnected) {
                            tab.password_attempt = None;
                            disconnected.push(tab.id.clone());
                        }
                        if state == ssh::State::Connected && !tab.closing {
                            if tab.reconnect.take().is_some() {
                                tab.error = None;
                                if let TabContent::Sftp {
                                    files: Some(files), ..
                                } = &tab.content
                                {
                                    files.update(cx, |files, cx| files.reconnected(cx));
                                }
                            }
                            match &mut tab.content {
                                TabContent::Terminal(_) if !tab.counted => {
                                    tab.counted = true;
                                    successful_shells.push(tab.connection.id.clone());
                                }
                                TabContent::Sftp { files, .. } if files.is_none() => {
                                    if let Some(transport) = &tab.transport {
                                        let commands = transport.commands.clone();
                                        let runtime = self.runtime.clone();
                                        *files = Some(cx.new(|cx| {
                                            super::files::Files::new(
                                                &tab.connection,
                                                commands,
                                                runtime,
                                                self.store
                                                    .local_directory(&tab.connection.id)
                                                    .map(std::path::Path::to_path_buf),
                                                tab.id.clone(),
                                                self.preference_sender.clone(),
                                                cx,
                                            )
                                        }));
                                    }
                                }
                                _ => (),
                            }
                        }
                        if matches!(
                            state,
                            ssh::State::Connected | ssh::State::Failed | ssh::State::Disconnected
                        ) && let Some(dialog) = tab.dialog.take()
                        {
                            let password = dialog
                                .update(cx, |dialog, window, _| {
                                    let password = dialog.take_password();
                                    dialog.close(window);
                                    password
                                })
                                .ok()
                                .flatten();
                            if state == ssh::State::Connected
                                && !tab.closing
                                && tab.save_password
                                && let Some(password) = password
                            {
                                passwords
                                    .push(PasswordUpdate::Save(tab.connection.clone(), password));
                            }
                        }
                        if matches!(
                            state,
                            ssh::State::Connected | ssh::State::Failed | ssh::State::Disconnected
                        ) && let Some(dialog) = tab.host_dialog.take()
                        {
                            dialog.dismiss(self.owner, cx);
                        }
                        if state == ssh::State::Connected
                            && !tab.closing
                            && self.settings.is_none()
                            && active_id.as_ref() == Some(&tab.id)
                        {
                            let focus = tab.visible_focus(cx);
                            let _ = self.owner.update(cx, |_, window, _| window.focus(&focus));
                        }
                    }
                    ssh::Event::Error(error) => tab.error = Some(error),
                    ssh::Event::Exit(status) => {
                        tab.error = Some(format!("Remote shell exited with status {status}"))
                    }
                    ssh::Event::Output(_) => (),
                }
            }
            if let Ok(prompt) = tab.prompts.try_recv() {
                if tab.closing || !super::auth_dialog::is_current(&prompt, tab.state) {
                    continue;
                }
                changed = true;
                tab.handle_prompt(prompt, &self.store, self.owner, &mut passwords, cx);
            }
        }
        for password in passwords {
            self.save_preference("password", move |store| password.apply(store), cx);
        }
        for id in successful_shells {
            self.save_preference("usage", move |store| store.record_successful_shell(&id), cx);
        }
        for id in disconnected {
            if let Some(index) = self.tabs.iter().position(|tab| tab.id == id) {
                self.disconnect(index, cx);
            }
        }
        if changed {
            cx.notify();
        }
    }

    pub(super) fn poll_local_preferences(&mut self, cx: &mut Context<Self>) {
        while let Ok(preference) = self.preference_events.try_recv() {
            if self.tabs.iter().any(|tab| {
                !tab.closing
                    && tab.id == preference.tab_id
                    && tab.connection.id == preference.connection_id
                    && matches!(tab.content, TabContent::Sftp { files: Some(_), .. })
            }) {
                self.save_preference(
                    "local directory",
                    move |store| {
                        store.set_local_directory(&preference.connection_id, &preference.path)
                    },
                    cx,
                );
            }
        }
    }
}
