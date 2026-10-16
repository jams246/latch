use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc as sync,
};
use std::time::Duration;

use latch::{
    sftp::Transfer,
    ssh,
    terminal::{TerminalInput, TerminalState},
};
use tokio::{sync::mpsc, task::JoinHandle};

pub(super) struct Transport {
    pub commands: mpsc::Sender<ssh::Command>,
    _session: ssh::SessionControl,
    router: JoinHandle<()>,
    accepting_input: Arc<AtomicBool>,
}

impl Transport {
    /// Keeps protocol output flowing independently of the terminal's visible lifetime.
    pub fn new(
        session: ssh::SessionControl,
        events: mpsc::Receiver<ssh::Event>,
        input: mpsc::UnboundedReceiver<TerminalInput>,
        state: impl Into<Option<TerminalState>>,
        runtime: &tokio::runtime::Runtime,
    ) -> (Self, sync::Receiver<ssh::Event>) {
        let commands = session.commands.clone();
        let accepting_input = Arc::new(AtomicBool::new(true));
        let (ui_events, receiver) = sync::channel();
        let router = runtime.spawn(route(
            events,
            input,
            state.into(),
            commands.clone(),
            ui_events,
            accepting_input.clone(),
        ));
        (
            Self {
                commands,
                _session: session,
                router,
                accepting_input,
            },
            receiver,
        )
    }

    /// Discards further user input while transport output remains available for cleanup.
    pub fn stop_input(&self) {
        self.accepting_input.store(false, Ordering::Relaxed);
    }

    /// Finishes transfer cancellation before disconnecting and releasing the transport.
    pub async fn shutdown(mut self, transfer: Option<Transfer>) -> Result<(), String> {
        self.stop_input();
        let cleanup = match transfer {
            Some(transfer) => transfer
                .cancel_and_wait()
                .await
                .map_err(|error| error.to_string()),
            None => Ok(()),
        };
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            self.commands.send(ssh::Command::Disconnect),
        )
        .await;
        let shutdown = tokio::time::timeout(Duration::from_secs(5), &mut self.router).await;
        cleanup.and(match shutdown {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err("SSH I/O task stopped unexpectedly during shutdown".into()),
            Err(_) => Err("SSH shutdown timed out; the transport was closed".into()),
        })
    }
}

impl Drop for Transport {
    fn drop(&mut self) {
        self.router.abort();
        // SessionControl closes its socket when this owner leaves scope.
    }
}

async fn route(
    mut events: mpsc::Receiver<ssh::Event>,
    mut input: mpsc::UnboundedReceiver<TerminalInput>,
    state: Option<TerminalState>,
    commands: mpsc::Sender<ssh::Command>,
    ui_events: sync::Sender<ssh::Event>,
    accepting_input: Arc<AtomicBool>,
) {
    let mut input_open = true;
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(ssh::Event::Output(bytes)) => {
                    if let Some(state) = &state {
                        let reply = state.feed(&bytes);
                        if !reply.is_empty() && commands.send(ssh::Command::Input(reply)).await.is_err() { break; }
                    }
                }
                Some(event) => {
                    if matches!(event, ssh::Event::Status(ssh::State::Reconnecting))
                        && let Some(state) = &state
                    {
                        state.reset_for_reconnect();
                    }
                    let _ = ui_events.send(event);
                }
                None => break,
            },
            event = input.recv(), if input_open => match event {
                Some(event) if accepting_input.load(Ordering::Relaxed) => {
                    let command = match event {
                        TerminalInput::Data(bytes) => ssh::Command::Input(bytes),
                        TerminalInput::Resize { columns, rows } => ssh::Command::Resize { cols: columns.into(), rows: rows.into() },
                    };
                    if commands.send(command).await.is_err() { break; }
                }
                Some(_) => (),
                None => input_open = false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn router_should_keep_protocol_replies_alive_after_ui_receivers_close() {
        let (event_tx, events) = mpsc::channel(4);
        let (input_tx, input) = mpsc::unbounded_channel();
        let (commands, mut command_rx) = mpsc::channel(4);
        let (ui_events, ui_rx) = sync::channel();
        drop(input_tx);
        drop(ui_rx);
        let task = tokio::spawn(route(
            events,
            input,
            Some(TerminalState::new(80, 24)),
            commands,
            ui_events,
            Arc::new(AtomicBool::new(false)),
        ));
        event_tx
            .send(ssh::Event::Status(ssh::State::Connected))
            .await
            .unwrap();
        event_tx
            .send(ssh::Event::Output(b"abc\x1b[6n".to_vec()))
            .await
            .unwrap();
        let command = tokio::time::timeout(Duration::from_secs(1), command_rx.recv())
            .await
            .unwrap();
        assert!(matches!(command, Some(ssh::Command::Input(bytes)) if bytes == b"\x1b[1;4R"));
        drop(event_tx);
        task.await.unwrap();
    }

    #[tokio::test]
    async fn cleanup_should_stop_user_input_without_stopping_output() {
        let (event_tx, events) = mpsc::channel(4);
        let (input_tx, input) = mpsc::unbounded_channel();
        let (commands, mut command_rx) = mpsc::channel(4);
        let (ui_events, _ui_rx) = sync::channel();
        let task = tokio::spawn(route(
            events,
            input,
            Some(TerminalState::new(80, 24)),
            commands,
            ui_events,
            Arc::new(AtomicBool::new(false)),
        ));
        input_tx
            .send(TerminalInput::Data(b"ignored".to_vec()))
            .unwrap();
        drop(input_tx);
        event_tx
            .send(ssh::Event::Output(b"\x1b[6n".to_vec()))
            .await
            .unwrap();
        drop(event_tx);
        task.await.unwrap();
        assert!(
            matches!(command_rx.recv().await, Some(ssh::Command::Input(bytes)) if bytes == b"\x1b[1;1R")
        );
        assert!(command_rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn sftp_router_relays_status_without_terminal_output_replies() {
        let (event_tx, events) = mpsc::channel(4);
        let (input_tx, input) = mpsc::unbounded_channel();
        let (commands, mut command_rx) = mpsc::channel(4);
        let (ui_events, ui_rx) = sync::channel();
        drop(input_tx);
        let task = tokio::spawn(route(
            events,
            input,
            None,
            commands,
            ui_events,
            Arc::new(AtomicBool::new(true)),
        ));
        event_tx
            .send(ssh::Event::Status(ssh::State::Connected))
            .await
            .unwrap();
        event_tx
            .send(ssh::Event::Output(b"\x1b[6n".to_vec()))
            .await
            .unwrap();
        drop(event_tx);
        task.await.unwrap();
        assert!(matches!(
            ui_rx.try_recv(),
            Ok(ssh::Event::Status(ssh::State::Connected))
        ));
        assert!(command_rx.recv().await.is_none());
    }
}
