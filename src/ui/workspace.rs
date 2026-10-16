use super::{Latch, SessionKind, TabContent, button};
use gpui::{prelude::*, *};
use latch::ssh::State;

impl Latch {
    pub(super) fn workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = super::theme::CARBON;
        let reconnect = self.reconnect_view(window, cx);
        if let Some(index) = self.active.filter(|i| *i < self.tabs.len()) {
            let tab = &self.tabs[index];
            let connection = tab.connection.clone();
            let waiting_focus = match &tab.content {
                TabContent::Sftp { files: None, focus } => Some(focus),
                _ => None,
            };
            div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .when_some(waiting_focus, |view, focus| view.track_focus(focus))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .p_2()
                        .bg(rgb(colors.panel))
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
                                .when(matches!(tab.content, TabContent::Terminal(_)), |actions| {
                                    actions.child(button("files", "SFTP").on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            let Some(tab) = this.tabs.get(index) else {
                                                return;
                                            };
                                            if tab.state != State::Connected || tab.closing {
                                                return;
                                            }
                                            this.connect(
                                                connection.clone(),
                                                SessionKind::Sftp,
                                                window,
                                                cx,
                                            );
                                        },
                                    )))
                                })
                                .child(button("disconnect", "Disconnect").on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.close_tab(index, window, cx);
                                    },
                                ))),
                        ),
                )
                .children(
                    tab.error
                        .clone()
                        .filter(|_| tab.reconnect.is_none())
                        .map(|error| div().p_3().text_color(rgb(colors.danger)).child(error)),
                )
                .child(
                    div().flex_1().min_h_0().child(match &tab.content {
                        TabContent::Terminal(terminal) => terminal.clone().into_any_element(),
                        TabContent::Sftp {
                            files: Some(files), ..
                        } => files.clone().into_any_element(),
                        TabContent::Sftp { files: None, .. } => div()
                            .p_3()
                            .child(match tab.state {
                                State::Connected => "Opening SFTP…",
                                State::Connecting | State::Authenticating => "Connecting SFTP…",
                                State::Reconnecting => "Reconnecting SFTP…",
                                State::Failed | State::Disconnected => "SFTP session closed",
                            })
                            .into_any_element(),
                    }),
                )
                .child(reconnect)
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
                .child("Save a connection. Open a terminal or SFTP tab.")
                .child(
                    div()
                        .text_color(rgb(colors.muted))
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
        State::Reconnecting => "Reconnecting…",
        State::Authenticating => "Authenticating…",
        State::Connected => "Connected",
        State::Failed => "Failed",
    }
}
