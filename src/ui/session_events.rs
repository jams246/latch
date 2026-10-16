use super::{Latch, TabContent, auth_dialog::AuthDialog, options};
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
                        ssh::State::Connecting | ssh::State::Authenticating
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
                        if matches!(state, ssh::State::Failed | ssh::State::Disconnected) {
                            disconnected.push(tab.id.clone());
                        }
                        if state == ssh::State::Connected && !tab.closing {
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
                            let _ = dialog.update(cx, |dialog, window, _| dialog.close(window));
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
                if let ssh::Prompt::HostKey {
                    host,
                    port,
                    algorithm,
                    fingerprint,
                    reply,
                } = prompt
                {
                    tab.host_dialog = Some(super::host_dialog::HostDialog::new(
                        format!(
                            "Unknown host {host}:{port}\n{algorithm}\n{fingerprint}\nVerify this fingerprint out of band before trusting."
                        ),
                        reply,
                        cx,
                    ));
                    continue;
                }
                if let Some(dialog) = tab.dialog {
                    let _ = dialog.update(cx, |dialog, window, cx| {
                        dialog.set_prompt(prompt, window, cx)
                    });
                } else {
                    let destination = tab.destination.clone();
                    let Some(transport) = &tab.transport else {
                        continue;
                    };
                    let commands = transport.commands.clone();
                    let cancellation = commands.clone();
                    let owner = self.owner;
                    let dialog_options = options("SSH authentication", 500., 400., cx);
                    match cx.open_window(dialog_options, |window, cx| {
                        let modal = super::modal::attach(window, owner, cx);
                        cx.new(|cx| {
                            AuthDialog::new(
                                prompt,
                                &tab.name,
                                destination,
                                commands,
                                modal,
                                window,
                                cx,
                            )
                        })
                    }) {
                        Ok(dialog) => tab.dialog = Some(dialog),
                        Err(error) => {
                            tab.error = Some(format!("Cannot open authentication dialog: {error}"));
                            let _ = cancellation.try_send(ssh::Command::Disconnect);
                        }
                    }
                }
            }
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
