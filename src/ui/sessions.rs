#[cfg(test)]
pub(super) mod tests;

use super::{Latch, SessionKind, Tab, TabContent, transport::Transport};
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
        kind: SessionKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shutting_down || self.data_busy {
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
        let session = match kind {
            SessionKind::Terminal => ssh::spawn(config, prompt_tx, event_tx),
            SessionKind::Sftp => ssh::spawn_sftp(config, prompt_tx, event_tx),
        };
        drop(guard);
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let (content, state) = match kind {
            SessionKind::Terminal => {
                let state = TerminalState::new(80, 24);
                let terminal = cx.new(|cx| {
                    TerminalView::new(
                        state.clone(),
                        Arc::new(move |input| {
                            let _ = input_tx.send(input);
                        }),
                        cx,
                    )
                });
                (TabContent::Terminal(terminal), Some(state))
            }
            SessionKind::Sftp => (
                TabContent::Sftp {
                    files: None,
                    focus: cx.focus_handle(),
                },
                None,
            ),
        };
        let (transport, events) = Transport::new(session, event_rx, input_rx, state, &self.runtime);
        let tab = Tab {
            id: uuid::Uuid::new_v4().to_string(),
            name: match kind {
                SessionKind::Terminal => connection.name.clone(),
                SessionKind::Sftp => format!("{} · SFTP", connection.name),
            },
            destination: format!(
                "{}@{}:{}",
                connection.username, connection.host, connection.port
            ),
            connection,
            state: ssh::State::Connecting,
            error: None,
            counted: false,
            transport: Some(transport),
            content,
            dialog: None,
            host_dialog: None,
            reconnect: None,
            events,
            prompts,
            closing: false,
        };
        window.focus(&tab.visible_focus(cx));
        self.tabs.push(tab);
        self.active = Some(self.tabs.len() - 1);
        cx.notify();
    }
}
