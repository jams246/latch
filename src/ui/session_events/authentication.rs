use crate::ui::{Latch, Tab, auth_dialog::AuthDialog, options};
use gpui::*;
use latch::{
    ssh,
    storage::{Connection, Store},
};

#[cfg(test)]
mod tests;

pub(super) enum PasswordUpdate {
    Save(Connection, ssh::Secret),
    Rejected(Connection, ssh::Secret),
}

impl PasswordUpdate {
    pub(super) fn apply(self, store: &mut Store) -> std::io::Result<()> {
        match self {
            Self::Save(connection, password) => store.save_password(&connection, password),
            Self::Rejected(connection, password)
                if store.saved_password(&connection) == Some(&password) =>
            {
                store.clear_password(&connection.id)
            }
            Self::Rejected(..) => Ok(()),
        }
    }
}

impl Tab {
    pub(super) fn handle_prompt(
        &mut self,
        prompt: ssh::Prompt,
        saved_password: Option<ssh::Secret>,
        owner: AnyWindowHandle,
        passwords: &mut Vec<PasswordUpdate>,
        cx: &mut Context<Latch>,
    ) {
        if let ssh::Prompt::Secret {
            kind: ssh::SecretKind::Password,
            error,
            ..
        } = &prompt
        {
            if error.is_some() {
                if let Some(password) = self.password_attempt.take() {
                    passwords.push(PasswordUpdate::Rejected(self.connection.clone(), password));
                }
            } else if let Some(password) = saved_password {
                self.password_attempt = Some(password.clone());
                if let ssh::Prompt::Secret { reply, .. } = prompt {
                    let _ = reply.send(Some(password.clone()));
                }
                return;
            }
        }
        if let ssh::Prompt::HostKey {
            host,
            port,
            algorithm,
            fingerprint,
            reply,
        } = prompt
        {
            self.host_dialog = Some(crate::ui::host_dialog::HostDialog::new(
                format!(
                    "Unknown host {host}:{port}\n{algorithm}\n{fingerprint}\nVerify this fingerprint out of band before trusting."
                ),
                reply,
                cx,
            ));
            return;
        }
        if let Some(dialog) = self.dialog {
            let _ = dialog.update(cx, |dialog, window, cx| {
                dialog.set_prompt(prompt, window, cx)
            });
        } else {
            let destination = self.destination.clone();
            let Some(transport) = &self.transport else {
                return;
            };
            let commands = transport.commands.clone();
            let cancellation = commands.clone();
            let dialog_options = options("SSH authentication", 500., 400., cx);
            match cx.open_window(dialog_options, |window, cx| {
                let modal = crate::ui::modal::attach(window, owner, cx);
                cx.new(|cx| {
                    AuthDialog::new(prompt, &self.name, destination, commands, modal, window, cx)
                })
            }) {
                Ok(dialog) => self.dialog = Some(dialog),
                Err(error) => {
                    self.error = Some(format!("Cannot open authentication dialog: {error}"));
                    let _ = cancellation.try_send(ssh::Command::Disconnect);
                }
            }
        }
    }
}
