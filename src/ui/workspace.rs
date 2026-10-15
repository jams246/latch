use super::{Latch, button};
use gpui::{prelude::*, *};
use latch::ssh::State;

impl Latch {
    pub(super) fn workspace(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(index) = self.active.filter(|i| *i < self.tabs.len()) {
            let tab = &self.tabs[index];
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .p_2()
                        .bg(rgb(0x1b2a38))
                        .child(format!(
                            "{} · {}",
                            tab.name,
                            if tab.closing {
                                "Disconnecting…"
                            } else {
                                state_label(tab.state)
                            }
                        ))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    button(
                                        "files",
                                        if tab.show_files { "Terminal" } else { "SFTP" },
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            let Some(tab) = this.tabs.get_mut(index) else {
                                                return;
                                            };
                                            if tab.state != State::Connected || tab.closing {
                                                return;
                                            }
                                            if tab.files.is_none() {
                                                let Some(transport) = &tab.transport else {
                                                    return;
                                                };
                                                let commands = transport.commands.clone();
                                                let runtime = this.runtime.clone();
                                                tab.files = Some(cx.new(|cx| {
                                                    super::files::Files::new(commands, runtime, cx)
                                                }));
                                            }
                                            tab.show_files = !tab.show_files;
                                            window.focus(&tab.visible_focus(cx));
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(button("disconnect", "Disconnect").on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.disconnect(index, cx);
                                        if let Some(tab) = this.tabs.get(index) {
                                            window.focus(&tab.visible_focus(cx));
                                        }
                                    },
                                ))),
                        ),
                )
                .children(
                    tab.error
                        .clone()
                        .map(|error| div().p_3().text_color(rgb(0xffa69b)).child(error)),
                )
                .child(div().flex_1().min_h_0().child(if tab.show_files {
                    tab.files.as_ref().map_or_else(
                        || div().into_any_element(),
                        |files| files.clone().into_any_element(),
                    )
                } else {
                    tab.terminal.clone().into_any_element()
                }))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3()
                .child(div().text_3xl().child("Latch"))
                .child("Save a connection. Open a secure shell.")
                .child(
                    div()
                        .text_color(rgb(0xa8c1d3))
                        .child("Independent tabs · Native terminal · SFTP"),
                )
                .into_any_element()
        }
    }
}

fn state_label(state: State) -> &'static str {
    match state {
        State::Disconnected => "Disconnected",
        State::Connecting => "Connecting…",
        State::Authenticating => "Authenticating…",
        State::Connected => "Connected",
        State::Failed => "Failed",
    }
}
