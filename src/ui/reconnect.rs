use super::{Latch, button, dialog_header};
use gpui::{prelude::*, *};
use latch::ssh::State;

#[cfg(test)]
mod tests;

pub(super) struct ReconnectDialog {
    focus: FocusHandle,
    close: FocusHandle,
}

impl ReconnectDialog {
    pub(super) fn new(cx: &mut App) -> Self {
        Self {
            focus: cx.focus_handle().tab_stop(false),
            close: cx.focus_handle().tab_index(0).tab_stop(true),
        }
    }
}

impl Latch {
    pub(super) fn reconnect_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(tab) = self.active.and_then(|index| self.tabs.get(index)) else {
            return div().into_any_element();
        };
        let Some(dialog) = &tab.reconnect else {
            return div().into_any_element();
        };
        let failed = matches!(tab.state, State::Failed | State::Disconnected);
        if !dialog.focus.contains_focused(window, cx) || (failed && dialog.focus.is_focused(window))
        {
            window.focus(if failed { &dialog.close } else { &dialog.focus });
        }
        let tab_id = tab.id.clone();
        let close_id = tab_id.clone();
        let colors = super::theme::CARBON;
        div()
            .id("reconnect-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(colors.overlay))
            .child(
                div()
                    .id("reconnect-dialog")
                    .debug_selector(|| "reconnect-dialog".into())
                    .track_focus(&dialog.focus)
                    .tab_group()
                    .w(px(460.))
                    .max_w_full()
                    .rounded_lg()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .bg(rgb(colors.surface))
                    .text_color(rgb(colors.text))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "tab"
                            && let Some(dialog) = this
                                .tabs
                                .iter()
                                .find(|tab| tab.id == tab_id)
                                .and_then(|tab| tab.reconnect.as_ref())
                        {
                            window.focus(if failed { &dialog.close } else { &dialog.focus });
                        }
                        cx.stop_propagation();
                    }))
                    .on_key_up(cx.listener(move |this, event: &KeyUpEvent, window, cx| {
                        if failed && event.keystroke.key == "enter" {
                            this.close_reconnect_tab(&close_id, window, cx);
                        }
                        cx.stop_propagation();
                    }))
                    .child(dialog_header(if failed {
                        "Reconnection failed"
                    } else {
                        "Reconnecting…"
                    }))
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(tab.name.clone())
                            .child(div().text_color(rgb(colors.muted)).child(if failed {
                                "Could not reconnect within 30 seconds. Close this tab to continue."
                            } else {
                                "The connection was interrupted. Trying to reconnect for up to 30 seconds…"
                            }))
                            .when(failed, |view| {
                                let tab_id = tab.id.clone();
                                view.child(
                                    div().flex().justify_end().child(
                                        button("close-reconnect", "Close")
                                            .debug_selector(|| "close-reconnect".into())
                                            .track_focus(&dialog.close)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.close_reconnect_tab(&tab_id, window, cx);
                                            })),
                                    ),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    fn close_reconnect_tab(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.iter().position(|tab| {
            tab.id == id
                && tab.reconnect.is_some()
                && matches!(tab.state, State::Failed | State::Disconnected)
        }) {
            self.close_tab(index, window, cx);
        }
    }
}
