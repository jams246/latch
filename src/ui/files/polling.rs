use super::*;

impl Files {
    pub(super) fn poll(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.poll_actions(cx);
        while let Ok(result) = self.local_events.try_recv() {
            self.local_busy = false;
            match result {
                Ok((directory, warning)) => {
                    self.selections[0].clear();
                    self.menu = None;
                    let changed = self.local.as_ref().is_none_or(|d| d.path != directory.path);
                    if let Some(index) = self.pending_local_history.take() {
                        self.local_history.commit_at(index, directory.path.clone());
                    } else {
                        self.local_history.commit(directory.path.clone());
                    }
                    self.local_status =
                        warning.unwrap_or_else(|| format!("{} entries", directory.entries.len()));
                    self.local = Some(directory);
                    if changed {
                        self.reveal_breadcrumbs(Side::Local);
                    }
                }
                Err(error) => {
                    self.pending_local_history = None;
                    self.local_status = format!("Cannot open local directory: {error}");
                }
            }
            cx.notify();
        }
        while let Ok(result) = self.events.try_recv() {
            self.busy = false;
            match result {
                Ok(RemoteResult::Directory(browser, directory)) => {
                    self.selections[1].clear();
                    self.menu = None;
                    let changed = self
                        .directory
                        .as_ref()
                        .is_none_or(|d| d.path != directory.path);
                    self.path.update(cx, |input, cx| {
                        input.take(cx);
                        input.replace(0..0, &directory.path, cx);
                    });
                    if let Some(index) = self.pending_remote_history.take() {
                        self.remote_history.commit_at(index, directory.path.clone());
                    } else {
                        self.remote_history.commit(directory.path.clone());
                    }
                    self.status = format!("{} entries", directory.entries.len());
                    self.directory = Some(directory);
                    if changed {
                        self.reveal_breadcrumbs(Side::Remote);
                    }
                    self.browser = Some(browser);
                }
                Ok(RemoteResult::FileLink(browser, path)) => {
                    self.browser = Some(browser);
                    self.pending_remote_history = None;
                    self.status = "Choose where to download the linked file".into();
                    let name = path.rsplit('/').next().unwrap_or("download").to_string();
                    self.download(path, &name, cx);
                }
                Err(error) => {
                    self.pending_remote_history = None;
                    self.status = error.to_string();
                    self.browser = None;
                }
            }
            cx.notify();
        }
        self.poll_transfer(cx);
    }

    fn poll_transfer(&mut self, cx: &mut Context<Self>) {
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
            let notice = if self.batch_copy && self.status != "Transfer complete" {
                format!(
                    "{} Completed items remain in the target directory.",
                    self.status
                )
            } else {
                self.status.clone()
            };
            self.refresh(Side::Local, cx);
            self.refresh(Side::Remote, cx);
            self.action_status[1] = Some(notice.clone());
            if self.batch_copy {
                self.action_status[0] = Some(notice);
            }
            self.batch_copy = false;
        }
    }
}

pub(super) fn transfer_status(event: TransferEvent) -> (String, bool) {
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
