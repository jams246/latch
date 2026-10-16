use super::*;

impl Files {
    fn start_transfer(&mut self, direction: Direction, cx: &mut Context<Self>) {
        if self.closing || self.action_busy || self.transfer.is_some() {
            return;
        }
        let Some(browser) = &self.browser else {
            eprintln!("Refresh the folder to reconnect before starting a transfer");
            return;
        };
        let (events, progress) = tokio::sync::mpsc::channel(32);
        let _guard = self.runtime.enter();
        self.transfer = Some(browser.transfer_interactive(direction, events));
        self.metrics = Some(operations::Metrics::new(std::time::Instant::now()));
        self.progress = Some(progress);
        cx.notify();
    }

    pub(in crate::ui) fn download(&mut self, remote: String, name: &str, cx: &mut Context<Self>) {
        if self.busy || self.action_busy || self.dialog.is_some() || self.transfer.is_some() {
            return;
        }
        let directory = self
            .local
            .as_ref()
            .map_or_else(|| PathBuf::from("."), |directory| directory.path.clone());
        let prompt = cx.prompt_for_new_path(&directory, Some(name));
        cx.spawn(async move |this, cx| match prompt.await {
            Ok(Ok(Some(local))) => {
                let _ = this.update(cx, |this, cx| {
                    this.start_transfer(Direction::Download { remote, local }, cx)
                });
            }
            Ok(Err(error)) => {
                eprintln!("Cannot choose download file: {error}");
            }
            _ => (),
        })
        .detach();
    }
}
