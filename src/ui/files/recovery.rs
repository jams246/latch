use super::*;

impl Files {
    pub(in crate::ui) fn connection_lost(&mut self, cx: &mut Context<Self>) {
        if self.closing || self.reconnecting {
            return;
        }
        self.reconnecting = true;
        self.reconnect_refresh = false;
        self.busy = true;
        self.menu = None;
        self.dialog = None;
        self.conflict = None;
        self.browser = None;
        self.pending_remote_history = None;
        if let Some(operation) = self.operation.take() {
            operation.abort();
        }
        (self.sender, self.events) = mpsc::channel();
        if self.action_side == Some(Side::Remote) {
            if let Some(task) = self.action_task.take() {
                task.abort();
            }
            (self.action_sender, self.action_events) = mpsc::channel();
            self.action_busy = false;
            self.action_side = None;
            self.error = Some(
                "File action interrupted. Some selected items may already have changed. Check the folder before retrying."
                    .into(),
            );
        }
        if let Some(transfer) = self.transfer.take() {
            transfer.cancel();
            let errors = self.cleanup_sender.clone();
            self.runtime.spawn(async move {
                if let Err(error) = transfer.cancel_and_wait().await {
                    let _ = errors.send(error);
                }
            });
            self.error = Some(
                "Transfer interrupted by connection loss. It will not resume automatically.".into(),
            );
        }
        self.progress = None;
        self.metrics = None;
        cx.notify();
    }

    pub(in crate::ui) fn reconnected(&mut self, cx: &mut Context<Self>) {
        if self.closing || !self.reconnecting {
            return;
        }
        self.reconnecting = false;
        self.busy = false;
        self.reconnect_refresh = true;
        self.poll_recovery(cx);
    }

    pub(super) fn request_reconnect(&mut self, cx: &mut Context<Self>) {
        if self.closing || self.reconnecting {
            return;
        }
        self.connection_lost(cx);
        let commands = self.commands.clone();
        self.runtime.spawn(async move {
            let _ = commands.send(ssh::Command::Reconnect).await;
        });
    }

    pub(super) fn poll_recovery(&mut self, cx: &mut Context<Self>) {
        while let Ok(error) = self.cleanup_events.try_recv() {
            let message = self.error.get_or_insert_with(String::new);
            message.push_str(&format!(" Transfer cleanup failed: {error}"));
            cx.notify();
        }
        if self.reconnect_refresh && !self.action_busy {
            self.reconnect_refresh = false;
            self.refresh(Side::Local, cx);
            self.refresh(Side::Remote, cx);
        }
        if !self.busy && !self.action_busy && self.browser.as_ref().is_some_and(Browser::is_closed)
        {
            self.request_reconnect(cx);
        }
    }
}

pub(super) fn connection_error(error: &sftp::Error) -> bool {
    use russh_sftp::{client::error::Error, protocol::StatusCode};
    match error {
        sftp::Error::NotConnected | sftp::Error::Timeout => true,
        sftp::Error::Remote(Error::Status(status)) => matches!(
            status.status_code,
            StatusCode::NoConnection | StatusCode::ConnectionLost
        ),
        sftp::Error::Remote(Error::Limited(_)) => false,
        sftp::Error::Remote(_) => true,
        _ => false,
    }
}
