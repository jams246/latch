use crate::ui::{Latch, LocalPreference, Tab, TabContent, files::Files};
use gpui::{AppContext, TestAppContext};
use latch::{ssh, storage::Store};
use std::{path::PathBuf, sync::Arc};

#[gpui::test]
fn only_matching_live_sftp_tabs_can_persist_local_preferences(cx: &mut TestAppContext) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/encrypted-storage/ui-preferences")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&directory).unwrap();
    let mut store = Store::load(directory.join("latch.db")).unwrap();
    let connection = latch::storage::Connection::new(
        "Synthetic SFTP".into(),
        "example.invalid".into(),
        "synthetic".into(),
        latch::storage::AuthMethod::Password,
    );
    store.upsert(connection.clone()).unwrap();
    store
        .set_local_directory(&connection.id, &directory)
        .unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx.add_window(|window, cx| {
        Latch::new(
            store,
            directory.clone(),
            runtime,
            window.window_handle(),
            cx,
        )
    });
    handle
        .update(cx, |app, _, cx| {
            let (commands, _) = tokio::sync::mpsc::channel(1);
            let files = cx.new(|cx| {
                Files::new(
                    &connection,
                    commands,
                    app.runtime.clone(),
                    Some(directory.clone()),
                    "live-tab".into(),
                    app.preference_sender.clone(),
                    cx,
                )
            });
            let (_, events) = std::sync::mpsc::channel();
            let (_, prompts) = tokio::sync::mpsc::channel(1);
            app.tabs.push(Tab {
                id: "live-tab".into(),
                connection: connection.clone(),
                name: connection.name.clone(),
                destination: "synthetic".into(),
                state: ssh::State::Connected,
                error: None,
                counted: false,
                transport: None,
                content: TabContent::Sftp {
                    files: Some(files),
                    focus: cx.focus_handle(),
                },
                dialog: None,
                host_dialog: None,
                events,
                prompts,
                closing: false,
            });

            for (tab_id, connection_id, closing) in [
                ("closed-tab", connection.id.as_str(), false),
                ("live-tab", "other-connection", false),
                ("live-tab", connection.id.as_str(), true),
            ] {
                app.tabs[0].closing = closing;
                app.preference_sender
                    .send(LocalPreference {
                        tab_id: tab_id.into(),
                        connection_id: connection_id.into(),
                        path: directory.join("ignored"),
                    })
                    .unwrap();
                app.poll(cx);
                assert_eq!(
                    app.store.local_directory(&connection.id),
                    Some(directory.as_path())
                );
            }

            app.tabs[0].closing = false;
            let accepted = directory.join("accepted");
            app.preference_sender
                .send(LocalPreference {
                    tab_id: "live-tab".into(),
                    connection_id: connection.id.clone(),
                    path: accepted.clone(),
                })
                .unwrap();
            app.poll(cx);

            app.tabs.clear();
            app.preference_sender
                .send(LocalPreference {
                    tab_id: "live-tab".into(),
                    connection_id: connection.id.clone(),
                    path: directory.join("late"),
                })
                .unwrap();
            app.poll(cx);
        })
        .unwrap();
    crate::ui::persistence::tests::settle(handle, cx);
    handle
        .read_with(cx, |app, _| {
            assert_eq!(
                app.store.local_directory(&connection.id),
                Some(directory.join("accepted").as_path())
            );
        })
        .unwrap();
}
