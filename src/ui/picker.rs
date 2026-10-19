use super::{Latch, SessionKind, button};
use gpui::{prelude::*, *};
use latch::storage::Connection;

pub(super) fn filter_connections<'a>(
    mut connections: Vec<&'a Connection>,
    search: &str,
) -> Vec<&'a Connection> {
    let search = search.trim().to_lowercase();
    if !search.is_empty() {
        connections.retain(|connection| {
            [&connection.name, &connection.host, &connection.username]
                .iter()
                .any(|value| value.to_lowercase().contains(&search))
        });
    }
    connections
}

impl Latch {
    pub(super) fn row(
        &self,
        connection: &Connection,
        section: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = super::theme::CARBON;
        let connect = connection.clone();
        let double_click = connection.clone();
        let sftp = connection.clone();
        let edit = connection.clone();
        let delete = connection.clone();
        let id = connection.id.clone();
        let name = connection.name.clone();
        let address = format!(
            "{}@{}:{}",
            connection.username, connection.host, connection.port
        );
        div()
            .id(SharedString::from(format!("{section}-{id}")))
            .debug_selector(|| format!("{section}-connection"))
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap_1()
            .p_2()
            .rounded_md()
            .bg(rgb(colors.surface))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.click_count() == 2 {
                    this.connect(double_click.clone(), SessionKind::Terminal, window, cx);
                }
            }))
            .child(
                div()
                    .id(SharedString::from(format!("details-{section}-{id}")))
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .min_w_0()
                    .child(
                        div()
                            .max_w(relative(0.5))
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(rgb(colors.muted))
                            .child(address.clone()),
                    )
                    .tooltip(move |_, cx| {
                        cx.new(|_| ConnectionTooltip(name.clone(), address.clone()))
                            .into()
                    }),
            )
            .child(
                div()
                    .id(SharedString::from(format!("actions-{section}-{id}")))
                    .flex()
                    .gap_1()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        button(
                            SharedString::from(format!("connect-{section}-{id}")),
                            "Terminal",
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.connect(connect.clone(), SessionKind::Terminal, window, cx)
                            },
                        )),
                    )
                    .child(
                        button(SharedString::from(format!("sftp-{section}-{id}")), "SFTP")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.connect(sftp.clone(), SessionKind::Sftp, window, cx)
                            })),
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
                        .text_color(rgb(colors.danger))
                        .debug_selector(move || format!("delete-{section}-{id}"))
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.request_connection_delete(delete.clone(), window, cx);
                            },
                        )),
                    ),
            )
            .into_any_element()
    }
}

struct ConnectionTooltip(String, String);

impl Render for ConnectionTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let colors = super::theme::CARBON;
        div()
            .max_w(px(600.))
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(colors.control))
            .border_1()
            .border_color(rgb(colors.border))
            .text_color(rgb(colors.text))
            .text_sm()
            .child(div().child(self.0.clone()))
            .child(div().text_color(rgb(colors.muted)).child(self.1.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::filter_connections;
    use latch::storage::{AuthMethod, Connection};

    fn connection(name: &str, host: &str, username: &str) -> Connection {
        Connection::new(
            name.into(),
            host.into(),
            username.into(),
            AuthMethod::Password,
        )
    }

    #[test]
    fn search_matches_name_host_and_username_without_case_sensitivity() {
        let connections = [
            connection("Production", "database.example", "alice"),
            connection("Backup", "PRODUCTION.example", "bob"),
            connection("Archive", "archive.example", "production-admin"),
            connection("Local", "localhost", "carol"),
        ];
        assert_eq!(
            filter_connections(connections.iter().collect(), "  PrOdUcTiOn  ")
                .iter()
                .map(|connection| connection.name.as_str())
                .collect::<Vec<_>>(),
            ["Production", "Backup", "Archive"]
        );
    }

    #[test]
    fn search_preserves_existing_order_and_intentional_duplicates() {
        let first = connection("Zulu", "shared.example", "alice");
        let second = connection("Alpha", "shared.example", "bob");
        let excluded = connection("Other", "other.example", "carol");
        assert_eq!(
            filter_connections(vec![&first, &excluded, &second, &first], "shared"),
            [&first, &second, &first]
        );
    }

    #[test]
    fn empty_search_shows_all_connections() {
        let connection = connection("Production", "server.example", "alice");
        assert_eq!(filter_connections(vec![&connection], "  "), [&connection]);
    }

    #[test]
    fn unmatched_search_returns_no_connections() {
        let connection = connection("Production", "server.example", "alice");
        assert!(filter_connections(vec![&connection], "missing").is_empty());
    }
}
