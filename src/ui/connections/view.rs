use super::*;

impl Latch {
    pub(in crate::ui) fn editor_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.editor else {
            return div().into_any_element();
        };
        let labels = [
            "Display name",
            "Hostname or IP address",
            "Port",
            "Username",
            "Existing private-key file path",
        ];
        div()
            .id("connection-editor")
            .track_focus(&editor.focus)
            .occlude()
            .tab_group()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000b0))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter"
                        if this.editor.as_ref().is_some_and(|editor| {
                            editor
                                .fields
                                .iter()
                                .any(|field| field.read(cx).focus.is_focused(window))
                        }) => {}
                    "escape" => {
                        this.close_editor(window, cx);
                    }
                    "tab" => {
                        if let Some(editor) = &this.editor {
                            editor.cycle_focus(event.keystroke.modifiers.shift, window, cx);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                if event.keystroke.key == "enter"
                    && this.editor.as_ref().is_some_and(|editor| {
                        editor
                            .fields
                            .iter()
                            .any(|field| field.read(cx).focus.is_focused(window))
                    })
                {
                    // Submit on release so restored button focus cannot activate Add again.
                    this.save_connection(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(510.))
                    .max_h_full()
                    .rounded_lg()
                    .overflow_hidden()
                    .bg(rgb(0x1c2c3a))
                    .child(crate::ui::dialog_header(if editor.original.is_some() {
                        "Edit connection"
                    } else {
                        "Add connection"
                    }))
                    .child(
                        div()
                            .id("editor-scroll")
                            .flex()
                            .flex_col()
                            .min_h_0()
                            .gap_2()
                            .p_4()
                            .overflow_y_scroll()
                            .children(
                                editor
                                    .fields
                                    .iter()
                                    .enumerate()
                                    .filter(|(i, _)| *i < 4 || editor.auth == 1)
                                    .map(|(i, field)| {
                                        div().flex().flex_col().gap_1().child(labels[i]).child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    div().flex_1().min_w_0().child(field.clone()),
                                                )
                                                .when(i == 4, |row| {
                                                    row.child(
                                                        button("browse-private-key", "Browse")
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.browse_key(cx)
                                                                },
                                                            )),
                                                    )
                                                }),
                                        )
                                    }),
                            )
                            .child(div().mt_2().child("Authentication"))
                            .child(
                                div().flex().flex_wrap().gap_2().children(
                                    ["Password", "Key file", "SSH agent", "Challenge"]
                                        .into_iter()
                                        .enumerate()
                                        .map(|(index, label)| {
                                            button(("auth-method", index), label)
                                                .bg(rgb(if editor.auth == index {
                                                    0x276453
                                                } else {
                                                    0x263c4d
                                                }))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    if let Some(editor) = &mut this.editor {
                                                        editor.auth = index;
                                                    }
                                                    cx.notify();
                                                }))
                                        }),
                                ),
                            )
                            .children(
                                self.error
                                    .clone()
                                    .map(|error| div().text_color(rgb(0xffa69b)).child(error)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .mt_3()
                                    .child(button("save", "Save connection").on_click(cx.listener(
                                        |this, _, window, cx| this.save_connection(window, cx),
                                    )))
                                    .child(
                                        button("cancel-edit", "Cancel")
                                            .track_focus(&editor.cancel)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.close_editor(window, cx);
                                            })),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
