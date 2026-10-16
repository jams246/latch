use super::*;

impl Render for AuthDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = crate::ui::theme::CARBON;
        let unlock = matches!(
            self.prompt,
            Some(Prompt::Secret {
                kind: SecretKind::KeyPassphrase { .. },
                ..
            })
        );
        div()
            .id("authentication")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.surface))
            .text_color(rgb(palette.text))
            .text_size(px(14.))
            .overflow_hidden()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter"
                        if this.focus.is_focused(window)
                            || this
                                .fields
                                .iter()
                                .any(|(_, field)| field.read(cx).focus.is_focused(window)) =>
                    {
                        this.submit(window, cx)
                    }
                    "escape" => this.cancel(window),
                    "tab" => {
                        if event.keystroke.modifiers.shift {
                            window.focus_prev();
                        } else {
                            window.focus_next();
                        }
                    }
                    _ => (),
                }
            }))
            .child(crate::ui::dialog_header(self.title.clone()))
            .child(
                div()
                    .id("authentication-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .p_4()
                    .overflow_y_scroll()
                    .child(self.destination.clone())
                    .child(
                        div()
                            .text_color(rgb(palette.muted))
                            .child(self.detail.clone()),
                    )
                    .children(
                        self.error
                            .clone()
                            .map(|error| div().text_color(rgb(palette.danger)).child(error)),
                    )
                    .children(self.fields.iter().map(|(label, input)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(label.clone())
                            .child(input.clone())
                    }))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .mt_2()
                            .child(
                                button(
                                    "submit",
                                    if self.busy {
                                        "Authenticating…"
                                    } else if unlock {
                                        "Unlock"
                                    } else {
                                        "Connect"
                                    },
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.submit(window, cx)),
                                ),
                            )
                            .child(
                                button("cancel", "Cancel").on_click(
                                    cx.listener(|this, _, window, _| this.cancel(window)),
                                ),
                            ),
                    ),
            )
    }
}
