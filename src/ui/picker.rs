use super::{Latch, button};
use gpui::{prelude::*, *};
use latch::storage::Connection;

impl Latch {
    pub(super) fn row(
        &self,
        connection: &Connection,
        section: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let connect = connection.clone();
        let edit = connection.clone();
        let id = connection.id.clone();
        div()
            .id(SharedString::from(format!("{section}-{id}")))
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .bg(rgb(0x1e2f3d))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(connection.name.clone()),
            )
            .child(div().text_xs().text_color(rgb(0xa8c1d3)).child(format!(
                "{}@{}:{}",
                connection.username, connection.host, connection.port
            )))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .mt_1()
                    .child(
                        button(
                            SharedString::from(format!("connect-{section}-{id}")),
                            "Connect",
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.connect(connect.clone(), window, cx),
                        )),
                    )
                    .child(
                        button(SharedString::from(format!("edit-{section}-{id}")), "Edit")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit(Some(edit.clone()), window, cx)
                            })),
                    )
                    .child(
                        button(
                            SharedString::from(format!("delete-{section}-{id}")),
                            "Delete",
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Err(error) = this.store.delete(&id) {
                                this.error = Some(format!("Cannot delete connection: {error}"));
                            }
                            cx.notify();
                        })),
                    ),
            )
            .into_any_element()
    }
}
