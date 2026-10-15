use super::{Latch, Tab, transport::Transport};
use gpui::*;
use latch::{
    ssh, storage,
    terminal::{TerminalState, TerminalView},
};
use std::sync::Arc;
use tokio::sync::mpsc;

impl Latch {
    pub(super) fn connect(
        &mut self,
        connection: storage::Connection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shutting_down {
            return;
        }
        let config = ssh::ConnectionConfig {
            name: connection.name.clone(),
            host: connection.host.clone(),
            port: connection.port,
            username: connection.username.clone(),
            known_hosts: self.directory.join("known_hosts"),
            cols: 80,
            rows: 24,
            auth: match &connection.auth {
                storage::AuthMethod::Password => ssh::AuthMethod::Password,
                storage::AuthMethod::KeyFile { path } => ssh::AuthMethod::KeyFile(path.clone()),
                storage::AuthMethod::Agent => ssh::AuthMethod::Agent,
                storage::AuthMethod::KeyboardInteractive => ssh::AuthMethod::KeyboardInteractive,
            },
        };
        let (prompt_tx, prompts) = mpsc::channel(1);
        let (event_tx, event_rx) = mpsc::channel(128);
        let guard = self.runtime.enter();
        let session = ssh::spawn(config, prompt_tx, event_tx);
        drop(guard);
        let state = TerminalState::new(80, 24);
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let terminal = cx.new(|cx| {
            TerminalView::new(
                state.clone(),
                Arc::new(move |input| {
                    let _ = input_tx.send(input);
                }),
                cx,
            )
        });
        let (transport, events) = Transport::new(session, event_rx, input_rx, state, &self.runtime);
        window.focus(&terminal.focus_handle(cx));
        self.tabs.push(Tab {
            id: uuid::Uuid::new_v4().to_string(),
            connection_id: connection.id,
            name: connection.name,
            destination: format!(
                "{}@{}:{}",
                connection.username, connection.host, connection.port
            ),
            state: ssh::State::Connecting,
            error: None,
            counted: false,
            transport: Some(transport),
            terminal,
            dialog: None,
            events,
            prompts,
            files: None,
            show_files: false,
            closing: false,
        });
        self.active = Some(self.tabs.len() - 1);
        cx.notify();
    }
}
