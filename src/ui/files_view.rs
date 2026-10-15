use super::{button, files::Files};
use gpui::{prelude::*, *};

impl Render for Files {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self
            .directory
            .as_ref()
            .map(|d| d.entries.clone())
            .unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_3()
            .gap_3()
            .bg(rgb(0x15232f))
            .child(div().text_lg().child("Remote files"))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .child(self.path.clone())
                            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                if event.keystroke.key == "enter" {
                                    this.navigate(this.path.read(cx).value.to_string(), cx);
                                }
                            })),
                    )
                    .child(button("go", "Go").on_click(cx.listener(|this, _, _, cx| {
                        this.navigate(this.path.read(cx).value.to_string(), cx)
                    })))
                    .child(button("up", "Up").on_click(cx.listener(|this, _, _, cx| {
                        let path = this.directory.as_ref().map_or("/".into(), |d| {
                            format!("{}/..", d.path.trim_end_matches('/'))
                        });
                        this.navigate(path, cx);
                    }))),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("upload", "Upload file…")
                            .on_click(cx.listener(|this, _, _, cx| this.upload(cx))),
                    )
                    .child(
                        button("refresh", "Refresh").on_click(cx.listener(|this, _, _, cx| {
                            this.navigate(this.path.read(cx).value.to_string(), cx)
                        })),
                    )
                    .when(self.transfer.is_some(), |row| {
                        row.child(button("cancel-transfer", "Cancel transfer").on_click(
                            cx.listener(|this, _, _, _| {
                                if let Some(transfer) = &this.transfer {
                                    transfer.cancel();
                                }
                            }),
                        ))
                    }),
            )
            .child(div().text_color(rgb(0xa8c1d3)).child(self.status.clone()))
            .child(
                div()
                    .id("remote-entries")
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(
                        rows.into_iter()
                            .enumerate()
                            .filter(|(_, entry)| !matches!(entry.name.as_str(), "." | ".."))
                            .map(|(index, entry)| {
                                let name = entry.name.clone();
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .justify_between()
                                    .p_2()
                                    .bg(rgb(0x1e2f3d))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .min_w_0()
                                            .child(format!(
                                                "{}{}",
                                                if entry.is_dir { "Folder: " } else { "" },
                                                name
                                            ))
                                            .when(!entry.is_dir, |row| {
                                                row.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(rgb(0xa8c1d3))
                                                        .child(entry.size.map_or_else(
                                                            || "Size unavailable".into(),
                                                            |size| format!("{size} bytes"),
                                                        )),
                                                )
                                            }),
                                    )
                                    .child(
                                        button(
                                            ("remote-file", index),
                                            if entry.is_dir { "Open" } else { "Download…" },
                                        )
                                        .flex_shrink_0()
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                if entry.is_dir {
                                                    this.navigate(entry.path.clone(), cx);
                                                } else {
                                                    this.download(entry.path.clone(), &name, cx);
                                                }
                                            }),
                                        ),
                                    )
                            }),
                    ),
            )
    }
}
