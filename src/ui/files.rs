mod transfers;

use super::input::Input;
use gpui::*;
use latch::{
    sftp::{self, Browser, Direction, Directory, TransferEvent},
    ssh,
};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
};

pub(super) struct Files {
    pub path: Entity<Input>,
    pub directory: Option<Directory>,
    pub status: String,
    pub busy: bool,
    pub transfer: Option<sftp::Transfer>,
    browser: Option<Browser>,
    runtime: Arc<tokio::runtime::Runtime>,
    events: mpsc::Receiver<Result<(Browser, Directory), sftp::Error>>,
    sender: mpsc::Sender<Result<(Browser, Directory), sftp::Error>>,
    progress: Option<tokio::sync::mpsc::Receiver<TransferEvent>>,
    operation: Option<tokio::task::JoinHandle<()>>,
    commands: tokio::sync::mpsc::Sender<ssh::Command>,
    closing: bool,
}

impl Drop for Files {
    fn drop(&mut self) {
        if let Some(task) = &self.operation {
            task.abort();
        }
    }
}

impl Files {
    pub fn new(
        commands: tokio::sync::mpsc::Sender<ssh::Command>,
        runtime: Arc<tokio::runtime::Runtime>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (sender, events) = mpsc::channel();
        let result_tx = sender.clone();
        let opening_commands = commands.clone();
        let operation = runtime.spawn(async move {
            let result = async {
                let browser = Browser::connect(&opening_commands).await?;
                let directory = browser.list(".").await?;
                Ok((browser, directory))
            }
            .await;
            let _ = result_tx.send(result);
        });
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(30)).await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            path: cx.new(|cx| Input::new(".", false, cx)),
            directory: None,
            status: "Opening SFTP…".into(),
            busy: true,
            transfer: None,
            browser: None,
            runtime,
            events,
            sender,
            progress: None,
            operation: Some(operation),
            commands,
            closing: false,
        }
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        while let Ok(result) = self.events.try_recv() {
            self.busy = false;
            match result {
                Ok((browser, directory)) => {
                    self.path.update(cx, |input, cx| {
                        input.take(cx);
                        input.replace(0..0, &directory.path, cx);
                    });
                    self.status = format!("{} entries", directory.entries.len());
                    self.directory = Some(directory);
                    self.browser = Some(browser);
                }
                Err(error) => {
                    self.status = error.to_string();
                    self.browser = None;
                }
            }
            cx.notify();
        }
        let mut finished = false;
        if let Some(progress) = &mut self.progress {
            while let Ok(event) = progress.try_recv() {
                let (status, terminal) = transfer_status(event);
                self.status = status;
                finished |= terminal;
                cx.notify();
            }
        }
        if let Some(event) = self
            .transfer
            .as_ref()
            .and_then(sftp::Transfer::terminal_event)
        {
            if matches!(&event, TransferEvent::Failed(_)) {
                self.browser = None;
            }
            let (status, terminal) = transfer_status(event);
            self.status = status;
            finished |= terminal;
            cx.notify();
        }
        if !finished
            && self
                .progress
                .as_ref()
                .is_some_and(|progress| progress.is_closed())
        {
            self.status =
                "Transfer status was interrupted. Refresh the folder before retrying.".into();
            self.browser = None;
            finished = true;
            cx.notify();
        }
        if finished {
            self.transfer = None;
            self.progress = None;
        }
    }

    pub(super) fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        if self.busy || self.closing || self.transfer.is_some() {
            return;
        }
        let browser = self.browser.clone();
        let commands = self.commands.clone();
        let events = self.sender.clone();
        self.busy = true;
        self.status = "Loading directory…".into();
        self.operation = Some(self.runtime.spawn(async move {
            let result = async {
                let browser = match browser {
                    Some(browser) => browser,
                    None => Browser::connect(&commands).await?,
                };
                let directory = browser.list(&path).await?;
                Ok((browser, directory))
            }
            .await;
            let _ = events.send(result);
        }));
        cx.notify();
    }

    pub(super) fn prepare_close(&mut self, cx: &mut Context<Self>) -> Option<sftp::Transfer> {
        self.closing = true;
        self.busy = true;
        if let Some(operation) = self.operation.take() {
            operation.abort();
        }
        self.progress = None;
        self.browser = None;
        cx.notify();
        self.transfer.take()
    }
}

fn transfer_status(event: TransferEvent) -> (String, bool) {
    match event {
        TransferEvent::Progress { completed, total } => (
            match total {
                Some(total) => format!("Transferring: {completed} / {total} bytes"),
                None => format!("Transferring: {completed} bytes"),
            },
            false,
        ),
        TransferEvent::Completed => ("Transfer complete".into(), true),
        TransferEvent::Cancelled => ("Transfer cancelled".into(), true),
        TransferEvent::Failed(error) => (format!("{error} Refresh folder to reconnect."), true),
    }
}

#[cfg(test)]
mod tests {
    use super::{TransferEvent, transfer_status};

    #[test]
    fn retained_terminal_states_release_transfer_controls_but_progress_does_not() {
        assert_eq!(
            transfer_status(TransferEvent::Completed),
            ("Transfer complete".into(), true)
        );
        assert_eq!(
            transfer_status(TransferEvent::Cancelled),
            ("Transfer cancelled".into(), true)
        );
        assert_eq!(
            transfer_status(TransferEvent::Failed("failure".into())),
            ("failure Refresh folder to reconnect.".into(), true)
        );
        assert!(
            !transfer_status(TransferEvent::Progress {
                completed: 1,
                total: Some(2)
            })
            .1
        );
    }
}
