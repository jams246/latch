mod key_picker;
#[cfg(test)]
mod tests;
mod view;

use super::{Latch, button, input::Input};
use gpui::{prelude::*, *};
use latch::storage::{AuthMethod, Connection};

pub(super) struct Editor {
    original: Option<Connection>,
    fields: Vec<Entity<Input>>,
    auth: usize,
    focus: FocusHandle,
    cancel: FocusHandle,
    previous_focus: Option<FocusHandle>,
}

impl Editor {
    fn cycle_focus(&self, backwards: bool, window: &mut Window, cx: &App) {
        if backwards {
            window.focus_prev();
        } else {
            window.focus_next();
        }
        if !self.focus.contains_focused(window, cx) {
            let focus = if backwards {
                self.cancel.clone()
            } else {
                self.fields[0].read(cx).focus.clone()
            };
            window.focus(&focus);
        }
    }
}

impl Latch {
    pub(super) fn edit(
        &mut self,
        connection: Option<Connection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let values = connection.as_ref().map_or_else(
            || {
                vec![
                    String::new(),
                    String::new(),
                    "22".into(),
                    String::new(),
                    String::new(),
                ]
            },
            |c| {
                vec![
                    c.name.clone(),
                    c.host.clone(),
                    c.port.to_string(),
                    c.username.clone(),
                    match &c.auth {
                        AuthMethod::KeyFile { path } => path.display().to_string(),
                        _ => String::new(),
                    },
                ]
            },
        );
        let auth = connection.as_ref().map_or(0, |c| match c.auth {
            AuthMethod::Password => 0,
            AuthMethod::KeyFile { .. } => 1,
            AuthMethod::Agent => 2,
            AuthMethod::KeyboardInteractive => 3,
        });
        let fields: Vec<_> = values
            .iter()
            .map(|value| cx.new(|cx| Input::new(value, false, cx)))
            .collect();
        let previous_focus = window.focused(cx);
        window.focus(&fields[0].read(cx).focus);
        self.editor = Some(Editor {
            original: connection,
            fields,
            auth,
            focus: cx.focus_handle().tab_stop(false),
            cancel: cx.focus_handle().tab_index(0).tab_stop(true),
            previous_focus,
        });
        self.error = None;
        cx.notify();
    }

    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else {
            return;
        };
        let values: Vec<_> = editor
            .fields
            .iter()
            .map(|field| field.read(cx).value.trim().to_string())
            .collect();
        let Ok(port) = values[2].parse::<u16>() else {
            self.error = Some("Port must be a number between 1 and 65535".into());
            cx.notify();
            return;
        };
        let auth = match editor.auth {
            1 => AuthMethod::KeyFile {
                path: values[4].clone().into(),
            },
            2 => AuthMethod::Agent,
            3 => AuthMethod::KeyboardInteractive,
            _ => AuthMethod::Password,
        };
        let mut connection = editor.original.clone().unwrap_or_else(|| {
            Connection::new(
                values[0].clone(),
                values[1].clone(),
                values[3].clone(),
                auth.clone(),
            )
        });
        connection.name.clone_from(&values[0]);
        connection.host.clone_from(&values[1]);
        connection.port = port;
        connection.username.clone_from(&values[3]);
        connection.auth = auth;
        match self.store.upsert(connection) {
            Ok(()) => self.close_editor(window, cx),
            Err(error) => {
                self.error = Some(format!("Cannot save connection: {error}"));
                cx.notify();
            }
        }
    }

    fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.editor.take().and_then(|editor| editor.previous_focus);
        self.error = None;
        if let Some(tab) = self.active.and_then(|index| self.tabs.get(index)) {
            window.focus(&tab.visible_focus(cx));
        } else if let Some(previous) = previous {
            window.focus(&previous);
        } else {
            window.blur();
        }
        cx.notify();
    }
}
