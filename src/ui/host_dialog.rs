use super::{Latch, button, dialog_header};
use gpui::{prelude::*, *};
use tokio::sync::oneshot;

#[cfg(test)]
mod tests;

pub(super) struct HostDialog {
    detail: String,
    pub(super) reply: oneshot::Sender<bool>,
    focus: FocusHandle,
    cancel: FocusHandle,
    confirm: FocusHandle,
    previous_focus: Option<FocusHandle>,
    shown: bool,
}

impl HostDialog {
    pub(super) fn new(detail: String, reply: oneshot::Sender<bool>, cx: &mut App) -> Self {
        Self {
            detail,
            reply,
            focus: cx.focus_handle().tab_stop(false),
            cancel: cx.focus_handle().tab_index(0).tab_stop(true),
            confirm: cx.focus_handle().tab_index(0).tab_stop(true),
            previous_focus: None,
            shown: false,
        }
    }

    pub(super) fn dismiss(self, owner: AnyWindowHandle, cx: &mut App) {
        let _ = owner.update(cx, |_, window, cx| {
            if self.focus.contains_focused(window, cx) {
                if let Some(previous) = self.previous_focus {
                    window.focus(&previous);
                } else {
                    window.blur();
                }
            }
        });
    }
}

impl Latch {
    fn respond_host_key(
        &mut self,
        tab_id: &str,
        accept: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = self
            .tabs
            .iter_mut()
            .find(|tab| {
                tab.id == tab_id && tab.host_dialog.as_ref().is_some_and(|dialog| dialog.shown)
            })
            .and_then(|tab| tab.host_dialog.take())
        else {
            return;
        };
        let _ = dialog.reply.send(accept);
        if let Some(previous) = dialog.previous_focus {
            window.focus(&previous);
        } else {
            window.blur();
        }
        cx.notify();
    }

    pub(super) fn host_dialog_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let index = self
            .tabs
            .iter()
            .position(|tab| tab.host_dialog.as_ref().is_some_and(|dialog| dialog.shown))
            .or_else(|| self.tabs.iter().position(|tab| tab.host_dialog.is_some()));
        let Some(tab) = index.and_then(|index| self.tabs.get_mut(index)) else {
            return div().absolute().into_any_element();
        };
        let Some(dialog) = &mut tab.host_dialog else {
            return div().absolute().into_any_element();
        };
        dialog.shown = true;
        let key_down_tab = tab.id.clone();
        let key_up_tab = tab.id.clone();
        let cancel_tab = tab.id.clone();
        let confirm_tab = tab.id.clone();
        let pointer_tab = tab.id.clone();
        if !dialog.focus.contains_focused(window, cx) {
            if dialog.previous_focus.is_none() {
                dialog.previous_focus = window.focused(cx);
            }
            window.focus(&dialog.cancel);
        }
        let palette = super::theme::CARBON;
        div()
            .id("host-key-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(palette.overlay))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    if let Some(dialog) = this
                        .tabs
                        .iter()
                        .find(|tab| tab.id == pointer_tab)
                        .and_then(|tab| tab.host_dialog.as_ref())
                        && !dialog.focus.contains_focused(window, cx)
                    {
                        window.focus(&dialog.cancel);
                    }
                }),
            )
            .child(
                div()
                    .id("host-key-dialog")
                    .debug_selector(|| "host-key-dialog".into())
                    .track_focus(&dialog.focus)
                    .tab_group()
                    .occlude()
                    .w(px(500.))
                    .max_w_full()
                    .max_h_full()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .overflow_hidden()
                    .bg(rgb(palette.surface))
                    .text_color(rgb(palette.text))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" => this.respond_host_key(&key_down_tab, false, window, cx),
                            "tab" => {
                                if event.keystroke.modifiers.shift {
                                    window.focus_prev();
                                } else {
                                    window.focus_next();
                                }
                                if let Some(dialog) = this
                                    .tabs
                                    .iter()
                                    .find(|tab| tab.id == key_down_tab)
                                    .and_then(|tab| tab.host_dialog.as_ref())
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
                    .on_key_up(cx.listener(move |this, event: &KeyUpEvent, window, cx| {
                        if event.keystroke.key == "enter"
                            && let Some(dialog) = this
                                .tabs
                                .iter()
                                .find(|tab| tab.id == key_up_tab)
                                .and_then(|tab| tab.host_dialog.as_ref())
                        {
                            let accept = dialog.confirm.is_focused(window);
                            if accept || dialog.cancel.is_focused(window) {
                                this.respond_host_key(&key_up_tab, accept, window, cx);
                            }
                        }
                        cx.stop_propagation();
                    }))
                    .child(dialog_header("Verify server identity"))
                    .child(
                        div()
                            .id("host-key-scroll")
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(tab.destination.clone())
                            .child(
                                div()
                                    .text_color(rgb(palette.muted))
                                    .child(dialog.detail.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .justify_end()
                                    .child(
                                        button("cancel-host-key", "Cancel")
                                            .track_focus(&dialog.cancel)
                                            .debug_selector(|| "cancel-host-key".into())
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.respond_host_key(
                                                    &cancel_tab,
                                                    false,
                                                    window,
                                                    cx,
                                                )
                                            })),
                                    )
                                    .child(
                                        button("confirm-host-key", "Trust and Connect")
                                            .track_focus(&dialog.confirm)
                                            .debug_selector(|| "confirm-host-key".into())
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.respond_host_key(
                                                    &confirm_tab,
                                                    true,
                                                    window,
                                                    cx,
                                                )
                                            })),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
