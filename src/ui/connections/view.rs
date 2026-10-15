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
                        }) =>
                    {
                        this.save_connection(window, cx)
                    }
                    "escape" => {
                        this.close_editor(window, cx);
                    }
                    "tab" => {
                        if let Some(editor) = &this.editor {
                            editor.cycle_focus(event.keystroke.modifiers.shift, window, cx);
                        }
                    }
                    _ => (),
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .id("editor-scroll")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_5()
                    .w(px(510.))
                    .max_h_full()
                    .overflow_y_scroll()
                    .rounded_lg()
                    .bg(rgb(0x1c2c3a))
                    .child(div().text_xl().mb_2().child(if editor.original.is_some() {
                        "Edit connection"
                    } else {
                        "Add connection"
                    }))
                    .children(
                        editor
                            .fields
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| *i < 4 || editor.auth == 1)
                            .map(|(i, field)| {
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(labels[i])
                                    .child(field.clone())
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
                            .child(button("save", "Save connection").on_click(
                                cx.listener(|this, _, window, cx| this.save_connection(window, cx)),
                            ))
                            .child(
                                button("cancel-edit", "Cancel")
                                    .track_focus(&editor.cancel)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_editor(window, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
