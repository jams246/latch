use super::{Latch, button};
use gpui::{prelude::*, *};
use latch::storage::Connection;

#[cfg(test)]
mod tests;

pub(super) struct DeleteDialog {
    connection: Connection,
    focus: FocusHandle,
    cancel: FocusHandle,
    confirm: FocusHandle,
    previous_focus: Option<FocusHandle>,
    error: Option<String>,
}

impl Latch {
    pub(super) fn request_connection_delete(
        &mut self,
        connection: Connection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cancel = cx.focus_handle().tab_index(0).tab_stop(true);
        let previous_focus = window.focused(cx);
        window.focus(&cancel);
        self.connection_delete = Some(DeleteDialog {
            connection,
            focus: cx.focus_handle().tab_stop(false),
            cancel,
            confirm: cx.focus_handle().tab_index(0).tab_stop(true),
            previous_focus,
            error: None,
        });
        cx.notify();
    }

    fn close_connection_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self
            .connection_delete
            .take()
            .and_then(|dialog| dialog.previous_focus);
        if let Some(previous) = previous {
            window.focus(&previous);
        } else {
            window.blur();
        }
        cx.notify();
    }

    fn confirm_connection_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.connection_delete else {
            return;
        };
        match self.store.delete(&dialog.connection.id) {
            Ok(()) => self.close_connection_delete(window, cx),
            Err(error) => {
                dialog.error = Some(format!("Cannot delete connection: {error}"));
                cx.notify();
            }
        }
    }

    pub(super) fn connection_delete_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(dialog) = &self.connection_delete else {
            return div().absolute().into_any_element();
        };
        div()
            .id("connection-delete-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000a0))
            .child(
                div()
                    .id("connection-delete-dialog")
                    .debug_selector(|| "connection-delete-dialog".into())
                    .track_focus(&dialog.focus)
                    .tab_group()
                    .occlude()
                    .w(px(370.))
                    .max_w_full()
                    .p_4()
                    .gap_3()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .bg(rgb(0x1c2c3a))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" => this.close_connection_delete(window, cx),
                            "tab" => {
                                if event.keystroke.modifiers.shift {
                                    window.focus_prev();
                                } else {
                                    window.focus_next();
                                }
                                if let Some(dialog) = &this.connection_delete
                                    && !dialog.focus.contains_focused(window, cx)
                                {
                                    window.focus(if event.keystroke.modifiers.shift {
                                        &dialog.confirm
                                    } else {
                                        &dialog.cancel
                                    });
                                }
                            }
                            _ => (),
                        }
                        cx.stop_propagation();
                    }))
                    .on_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                        if event.keystroke.key == "enter" {
                            if this
                                .connection_delete
                                .as_ref()
                                .is_some_and(|dialog| dialog.cancel.is_focused(window))
                            {
                                this.close_connection_delete(window, cx);
                            } else if this
                                .connection_delete
                                .as_ref()
                                .is_some_and(|dialog| dialog.confirm.is_focused(window))
                            {
                                this.confirm_connection_delete(window, cx);
                            }
                        }
                        cx.stop_propagation();
                    }))
                    .child(div().text_lg().child("Delete connection?"))
                    .child(format!("Delete \"{}\"?", dialog.connection.name))
                    .child(format!(
                        "{}@{}:{}",
                        dialog.connection.username, dialog.connection.host, dialog.connection.port
                    ))
                    .child(
                        div()
                            .text_color(rgb(0xffa69b))
                            .child("This cannot be undone."),
                    )
                    .children(
                        dialog
                            .error
                            .iter()
                            .map(|error| div().text_color(rgb(0xffa69b)).child(error.clone())),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .justify_end()
                            .child(
                                button("cancel-connection-delete", "Cancel")
                                    .track_focus(&dialog.cancel)
                                    .debug_selector(|| "cancel-connection-delete".into())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_connection_delete(window, cx)
                                    })),
                            )
                            .child(
                                button("confirm-connection-delete", "Delete")
                                    .track_focus(&dialog.confirm)
                                    .debug_selector(|| "confirm-connection-delete".into())
                                    .text_color(rgb(0xff7777))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_connection_delete(window, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
