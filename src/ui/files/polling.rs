use super::*;

impl Files {
    fn poll_conflict(&mut self, cx: &mut Context<Self>) {
        match self.transfer.as_ref().and_then(sftp::Transfer::conflict) {
            Some(request) => {
                if self
                    .conflict
                    .as_ref()
                    .is_none_or(|dialog| dialog.request.id != request.id)
                {
                    self.conflict = Some(conflicts::Dialog::new(request, cx));
                    cx.notify();
                }
            }
            None => {
                if self.conflict.take().is_some() {
                    cx.notify();
                }
            }
        }
    }

    pub(super) fn poll(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        self.poll_recovery(cx);
        self.poll_actions(cx);
        while let Ok(result) = self.local_events.try_recv() {
            self.local_busy = false;
            match result {
                Ok(directory) => {
                    self.directory_errors[0] = None;
                    let _ = self.preference_sender.send(LocalPreference {
                        tab_id: self.tab_id.clone(),
                        connection_id: self.connection_id.clone(),
                        path: directory.path.clone(),
                    });
                    self.selections[0].clear();
                    self.menu = None;
                    let changed = self.local.as_ref().is_none_or(|d| d.path != directory.path);
                    if let Some(index) = self.pending_local_history.take() {
                        self.local_history.commit_at(index, directory.path.clone());
                    } else {
                        self.local_history.commit(directory.path.clone());
                    }
                    self.local = Some(directory);
                    self.rebuild_order(Side::Local);
                    if changed {
                        self.reveal_breadcrumbs(Side::Local);
                    }
                }
                Err(error) => {
                    self.pending_local_history = None;
                    self.directory_errors[0] =
                        Some(format!("Cannot open local directory: {error}"));
                }
            }
            cx.notify();
        }
        while let Ok(result) = self.events.try_recv() {
            self.busy = false;
            match result {
                Ok(RemoteResult::Directory(browser, directory)) => {
                    self.directory_errors[1] = None;
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
                    self.directory = Some(directory);
                    self.rebuild_order(Side::Remote);
                    if changed {
                        self.reveal_breadcrumbs(Side::Remote);
                    }
                    self.browser = Some(browser);
                }
                Ok(RemoteResult::FileLink(browser, path)) => {
                    self.browser = Some(browser);
                    self.pending_remote_history = None;
                    let name = path.rsplit('/').next().unwrap_or("download").to_string();
                    self.download(path, &name, cx);
                }
                Err(error) => {
                    let connection_lost = recovery::connection_error(&error);
                    self.pending_remote_history = None;
                    self.directory_errors[1] =
                        Some(format!("Cannot open server directory: {error}"));
                    self.browser = None;
                    if connection_lost {
                        self.request_reconnect(cx);
                    }
                }
            }
            cx.notify();
        }
        self.poll_transfer(cx);
    }

    fn poll_transfer(&mut self, cx: &mut Context<Self>) {
        let connection_lost = self.browser.as_ref().is_some_and(Browser::is_closed);
        let mut finished = false;
        let mut latest = None;
        if let Some(progress) = &mut self.progress {
            while let Ok(event) = progress.try_recv() {
                if matches!(event, TransferEvent::Progress { .. }) {
                    latest = Some(event);
                } else {
                    if matches!(&event, TransferEvent::Failed(_)) {
                        self.browser = None;
                    }
                    finished |= transfer_finished(event, &mut self.error);
                    cx.notify();
                }
            }
        }
        if let Some(event) = self
            .transfer
            .as_ref()
            .and_then(sftp::Transfer::progress_event)
        {
            latest = Some(event);
        }
        if let Some(TransferEvent::Progress {
            name,
            completed,
            total,
        }) = latest
            && let Some(metrics) = &mut self.metrics
        {
            let now = std::time::Instant::now();
            metrics.set_paused(
                self.transfer
                    .as_ref()
                    .is_some_and(sftp::Transfer::is_paused),
                now,
            );
            metrics.update(name, completed, total, now);
        }
        if self.transfer.is_some() {
            cx.notify();
        }
        self.poll_conflict(cx);
        if !finished
            && let Some(event) = self
                .transfer
                .as_ref()
                .and_then(sftp::Transfer::terminal_event)
        {
            if matches!(&event, TransferEvent::Failed(_)) {
                self.browser = None;
            }
            finished = transfer_finished(event, &mut self.error);
            cx.notify();
        }
        if !finished
            && self
                .progress
                .as_ref()
                .is_some_and(|progress| progress.is_closed())
        {
            self.error =
                Some("Transfer status was interrupted. Refresh the folder before retrying.".into());
            self.browser = None;
            finished = true;
            cx.notify();
        }
        if finished {
            self.transfer = None;
            self.progress = None;
            self.metrics = None;
            self.conflict = None;
            if connection_lost {
                self.request_reconnect(cx);
            }
            self.refresh(Side::Local, cx);
            self.refresh(Side::Remote, cx);
        }
    }
}

pub(super) fn transfer_finished(event: TransferEvent, failure: &mut Option<String>) -> bool {
    match event {
        TransferEvent::Progress { .. } => false,
        TransferEvent::Completed | TransferEvent::Cancelled => true,
        TransferEvent::Failed(error) => {
            *failure = Some(format!(
                "SFTP transfer failed: {error} Refresh folder to reconnect."
            ));
            true
        }
    }
}
