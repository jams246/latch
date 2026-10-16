use super::*;

impl Files {
    fn start_transfer(&mut self, direction: Direction, cx: &mut Context<Self>) {
        if self.closing || self.action_busy || self.transfer.is_some() {
            return;
        }
        let Some(browser) = &self.browser else {
            self.error = Some("Refresh the folder to reconnect before starting a transfer".into());
            cx.notify();
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
        let prompt = cx.prompt_for_new_path(&directory, Some(download_name(name)));
        cx.spawn(async move |this, cx| {
            let result = prompt
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            match result {
                Ok(Some(local)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.start_transfer(Direction::Download { remote, local }, cx)
                    });
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.error = Some(format!("Cannot choose download file: {error}"));
                        cx.notify();
                    });
                }
                Ok(None) => (),
            }
        })
        .detach();
    }
}

fn download_name(name: &str) -> &str {
    // Server filenames must not become paths or device names in the native save dialog.
    if local::validate_name(name).is_ok() {
        name
    } else {
        "download"
    }
}

#[cfg(test)]
mod tests {
    use super::download_name;

    #[test]
    fn save_dialog_receives_only_safe_filename_suggestions() {
        for name in [
            "../escape",
            r"\\server\share\file",
            "C:secret",
            "file:stream",
            "NUL",
            "file.",
        ] {
            assert_eq!(download_name(name), "download");
        }
        assert_eq!(download_name("report.txt"), "report.txt");
    }
}
