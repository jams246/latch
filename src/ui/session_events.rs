use super::{Latch, auth_dialog::AuthDialog, options};
use gpui::*;
use latch::ssh;

impl Latch {
    pub(super) fn poll(&mut self, cx: &mut Context<Self>) {
        self.poll_cleanup(cx);
        let mut changed = false;
        let mut disconnected = Vec::new();
        let active_id = self
            .active
            .and_then(|index| self.tabs.get(index))
            .map(|tab| tab.id.clone());
        for tab in &mut self.tabs {
            while let Ok(event) = tab.events.try_recv() {
                changed = true;
                match event {
                    ssh::Event::Status(state) => {
                        tab.state = state;
                        if matches!(state, ssh::State::Failed | ssh::State::Disconnected) {
                            disconnected.push(tab.id.clone());
                        }
                        if state == ssh::State::Connected && !tab.counted {
                            tab.counted = true;
                            if let Err(error) =
                                self.store.record_successful_shell(&tab.connection_id)
                            {
                                self.error = Some(format!("Cannot save usage: {error}"));
                            }
                        }
                        if matches!(
                            state,
                            ssh::State::Connected | ssh::State::Failed | ssh::State::Disconnected
                        ) && let Some(dialog) = tab.dialog.take()
                        {
                            let _ = dialog.update(cx, |dialog, window, _| dialog.close(window));
                        }
                        if state == ssh::State::Connected
                            && !tab.closing
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
        for id in disconnected {
            if let Some(index) = self.tabs.iter().position(|tab| tab.id == id) {
                self.disconnect(index, cx);
            }
        }
        if changed {
            cx.notify();
        }
    }
}
