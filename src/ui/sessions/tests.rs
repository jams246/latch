use crate::ui::{Latch, SessionKind, Tab, TabContent, files::Files};
use gpui::{AppContext, Context, Focusable, TestAppContext, WindowHandle};
use latch::{
    ssh,
    storage::{AuthMethod, Connection, Store},
    terminal::{TerminalState, TerminalView},
};
use std::sync::{Arc, mpsc};

fn fixture(cx: &mut TestAppContext) -> (WindowHandle<Latch>, Connection) {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/app-updates/sftp-tabs")
        .join(uuid::Uuid::new_v4().to_string());
    let mut store = Store::load(directory.join("latch.json")).unwrap();
    let connection = Connection::new(
        "Synthetic server".into(),
        "example.invalid".into(),
        "tester".into(),
        AuthMethod::Password,
    );
    store.upsert(connection.clone()).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    (handle, connection)
}

fn synthetic_tab(
    connection: Connection,
    kind: SessionKind,
    cx: &mut Context<Latch>,
) -> (Tab, mpsc::Sender<ssh::Event>) {
    let content = match kind {
        SessionKind::Terminal => TabContent::Terminal(
            cx.new(|cx| TerminalView::new(TerminalState::new(80, 24), Arc::new(|_| {}), cx)),
        ),
        SessionKind::Sftp => TabContent::Sftp {
            files: None,
            focus: cx.focus_handle(),
        },
    };
    let (sender, events) = mpsc::channel();
    let (_, prompts) = tokio::sync::mpsc::channel(1);
    (
        Tab {
            id: uuid::Uuid::new_v4().to_string(),
            name: connection.name.clone(),
            destination: format!(
                "{}@{}:{}",
                connection.username, connection.host, connection.port
            ),
            connection,
            state: ssh::State::Connecting,
            error: None,
            counted: false,
            transport: None,
            content,
            dialog: None,
            events,
            prompts,
            closing: false,
        },
        sender,
    )
}

#[gpui::test]
fn connected_sftp_does_not_count_usage_and_shell_counts_only_once(cx: &mut TestAppContext) {
    let (handle, connection) = fixture(cx);
    handle
        .update(cx, |app, _, cx| {
            let (terminal, terminal_events) =
                synthetic_tab(connection.clone(), SessionKind::Terminal, cx);
            let (sftp, sftp_events) = synthetic_tab(connection, SessionKind::Sftp, cx);
            app.tabs.extend([terminal, sftp]);
            sftp_events
                .send(ssh::Event::Status(ssh::State::Connected))
                .unwrap();
            app.poll(cx);
            assert_eq!(app.store.connections()[0].successful_shells, 0);
            assert!(!app.tabs[1].counted);
            for _ in 0..2 {
                terminal_events
                    .send(ssh::Event::Status(ssh::State::Connected))
                    .unwrap();
            }
            app.poll(cx);
            assert_eq!(app.store.connections()[0].successful_shells, 1);
            assert!(app.tabs[0].counted);
        })
        .unwrap();
}

#[gpui::test]
fn closing_terminal_keeps_sftp_tab_and_focuses_its_path(cx: &mut TestAppContext) {
    let (handle, connection) = fixture(cx);
    handle
        .update(cx, |app, window, cx| {
            let (terminal, _) = synthetic_tab(connection.clone(), SessionKind::Terminal, cx);
            let (mut sftp, _) = synthetic_tab(connection, SessionKind::Sftp, cx);
            let id = sftp.id.clone();
            let (commands, _) = tokio::sync::mpsc::channel(1);
            let files = cx.new(|cx| Files::new(commands, app.runtime.clone(), cx));
            let path_focus = files.read(cx).path.focus_handle(cx);
            let TabContent::Sftp { files: slot, .. } = &mut sftp.content else {
                panic!("Expected SFTP tab");
            };
            *slot = Some(files.clone());
            sftp.state = ssh::State::Connected;
            app.tabs.extend([terminal, sftp]);
            app.active = Some(1);

            app.close_tab(0, window, cx);

            assert_eq!(app.tabs.len(), 1);
            assert_eq!(app.tabs[0].id, id);
            assert_eq!(app.active, Some(0));
            assert_eq!(app.tabs[0].state, ssh::State::Connected);
            assert!(!app.tabs[0].closing);
            assert!(matches!(
                &app.tabs[0].content,
                TabContent::Sftp { files: Some(current), .. } if current == &files
            ));
            assert!(path_focus.is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn closing_sftp_keeps_terminal_tab_and_focuses_terminal(cx: &mut TestAppContext) {
    let (handle, connection) = fixture(cx);
    handle
        .update(cx, |app, window, cx| {
            let (terminal, _) = synthetic_tab(connection.clone(), SessionKind::Terminal, cx);
            let (sftp, _) = synthetic_tab(connection, SessionKind::Sftp, cx);
            let id = terminal.id.clone();
            let terminal_focus = terminal.visible_focus(cx);
            app.tabs.extend([terminal, sftp]);
            app.active = Some(1);

            app.close_tab(1, window, cx);

            assert_eq!(app.tabs.len(), 1);
            assert_eq!(app.tabs[0].id, id);
            assert_eq!(app.active, Some(0));
            assert!(!app.tabs[0].closing);
            assert!(matches!(app.tabs[0].content, TabContent::Terminal(_)));
            assert!(terminal_focus.is_focused(window));
        })
        .unwrap();
}
