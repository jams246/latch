use super::{Latch, SessionKind};
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
        initial: Option<String>,
        cx: &Context<Self>,
    ) -> AnyElement {
        use super::{
            choices::Choice,
            controls::{Hint, icon, quiet},
        };
        let colors = super::theme::current();
        let shell = connection.clone();
        let sftp = connection.clone();
        let id = connection.id.clone();
        let name = connection.name.clone();
        let address = format!(
            "{}@{}:{}",
            connection.username, connection.host, connection.port
        );
        let menu_id = id.clone();
        div()
            .flex()
            .items_center()
            .h(px(39.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(colors.surface))
            .child(
                div()
                    .w(px(42.))
                    .flex_shrink_0()
                    .text_lg()
                    .text_color(rgb(colors.accent))
                    .child(initial.unwrap_or_default()),
            )
            .child(
                quiet(SharedString::from(format!("open-{section}-{id}")), "")
                    .debug_selector(move || format!("{section}-connection"))
                    .flex()
                    .items_center()
                    .h_full()
                    .px_1()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .w(relative(0.22))
                            .min_w(px(44.))
                            .truncate()
                            .font_family("Cascadia Mono")
                            .text_xs()
                            .text_color(rgb(colors.muted))
                            .child(connection.username.clone()),
                    )
                    .tooltip(move |_, cx| cx.new(|_| Hint(name.clone(), address.clone())).into())
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        if e.click_count() == 1 {
                            this.connect(shell.clone(), SessionKind::Terminal, window, cx)
                        }
                    })),
            )
            .child(
                quiet(SharedString::from(format!("sftp-{section}-{id}")), "")
                    .w(px(72.))
                    .h_full()
                    .flex()
                    .gap_1()
                    .items_center()
                    .justify_center()
                    .px_1()
                    .text_color(rgb(colors.accent))
                    .child(icon("\u{ed41}"))
                    .child("SFTP")
                    .debug_selector(move || format!("sftp-{section}"))
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        if e.click_count() == 1 {
                            this.connect(sftp.clone(), SessionKind::Sftp, window, cx)
                        }
                    })),
            )
            .child(
                quiet(SharedString::from(format!("manage-{section}-{id}")), "⋯")
                    .w(px(32.))
                    .px_1()
                    .text_center()
                    .debug_selector(move || format!("manage-{section}-{menu_id}"))
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        this.show_choices(
                            [Choice::Edit(id.clone()), Choice::Delete(id.clone())],
                            e.position(),
                            window,
                            cx,
                        )
                    })),
            )
            .into_any_element()
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
