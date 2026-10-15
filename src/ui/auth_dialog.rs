#[cfg(test)]
mod lifecycle_tests;
mod state;
mod view;

pub(super) use state::is_current;

use super::{button, input::Input};
use gpui::{prelude::*, *};
use latch::ssh::{self, Prompt, SecretKind};
use tokio::sync::mpsc;

pub struct AuthDialog {
    prompt: Option<Prompt>,
    fields: Vec<(String, Entity<Input>)>,
    title: String,
    authentication_title: String,
    destination: String,
    detail: String,
    error: Option<String>,
    busy: bool,
    commands: mpsc::Sender<ssh::Command>,
    focus: FocusHandle,
    modal: Option<super::modal::ModalGuard>,
}

impl AuthDialog {
    pub fn new(
        prompt: Prompt,
        name: &str,
        destination: String,
        commands: mpsc::Sender<ssh::Command>,
        modal: anyhow::Result<super::modal::ModalGuard>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut dialog = Self {
            prompt: None,
            fields: vec![],
            title: format!("Authenticate to {name}"),
            authentication_title: format!("Authenticate to {name}"),
            destination,
            detail: String::new(),
            error: None,
            busy: false,
            commands,
            focus: cx.focus_handle(),
            modal: None,
        };
        let commands = dialog.commands.clone();
        let entity = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = entity.update(cx, |dialog, _| {
                dialog.fields.clear();
                dialog.prompt = None;
                dialog.modal.take();
            });
            let _ = commands.try_send(ssh::Command::Disconnect);
            true
        });
        dialog.set_prompt(prompt, window, cx);
        match modal {
            Ok(modal) => dialog.modal = Some(modal),
            Err(error) => {
                dialog.prompt = None;
                dialog.fields.clear();
                dialog.busy = true;
                dialog.error = Some(format!(
                    "Cannot create secure authentication window: {error}"
                ));
                let _ = dialog.commands.try_send(ssh::Command::Disconnect);
            }
        }
        dialog
    }

    pub fn set_prompt(&mut self, prompt: Prompt, window: &mut Window, cx: &mut Context<Self>) {
        self.fields.clear();
        self.error = None;
        self.busy = false;
        self.title.clone_from(&self.authentication_title);
        match &prompt {
            Prompt::HostKey {
                host,
                port,
                algorithm,
                fingerprint,
                ..
            } => {
                self.title = "Verify server identity".into();
                self.detail = format!(
                    "Unknown host {host}:{port}\n{algorithm}\n{fingerprint}\nVerify this fingerprint out of band before trusting."
                );
            }
            Prompt::Secret { kind, error, .. } => {
                let (label, detail) = match kind {
                    SecretKind::Password => ("Password", "SSH login password".to_string()),
                    SecretKind::KeyPassphrase { path } => {
                        self.title = "Unlock SSH key".into();
                        ("Passphrase", path.display().to_string())
                    }
                };
                self.detail = detail;
                self.error.clone_from(error);
                self.fields
                    .push((label.into(), cx.new(|cx| Input::new("", true, cx))));
            }
            Prompt::KeyboardInteractive {
                name,
                instructions,
                fields,
                ..
            } => {
                self.title = "SSH keyboard-interactive authentication".into();
                self.detail = format!("{name}\n{instructions}");
                for field in fields {
                    self.fields.push((
                        field.label.clone(),
                        cx.new(|cx| Input::new("", !field.echo, cx)),
                    ));
                }
            }
        }
        if let Some((_, field)) = self.fields.first() {
            window.focus(&field.read(cx).focus);
        } else {
            window.focus(&self.focus);
        }
        self.prompt = Some(prompt);
        cx.notify();
    }

    fn submit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(prompt) = self.prompt.take() else {
            return;
        };
        match prompt {
            Prompt::HostKey { reply, .. } => {
                let _ = reply.send(true);
            }
            Prompt::Secret { reply, .. } => {
                let secret = self
                    .fields
                    .first()
                    .map(|(_, field)| field.update(cx, |field, cx| field.take(cx)));
                let _ = reply.send(secret);
            }
            Prompt::KeyboardInteractive { reply, .. } => {
                let responses = self
                    .fields
                    .iter()
                    .map(|(_, field)| field.update(cx, |field, cx| field.take(cx)))
                    .collect();
                let _ = reply.send(Some(responses));
            }
        }
        for (_, field) in &self.fields {
            field.update(cx, |field, _| field.disabled = true);
        }
        self.busy = true;
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window) {
        self.prompt.take();
        self.fields.clear();
        self.modal.take();
        let _ = self.commands.try_send(ssh::Command::Disconnect);
        window.remove_window();
    }

    pub fn close(&mut self, window: &mut Window) {
        self.fields.clear();
        self.prompt = None;
        self.modal.take();
        window.remove_window();
    }
}
