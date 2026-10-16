use super::*;

impl Files {
    fn start_transfer(&mut self, direction: Direction, cx: &mut Context<Self>) {
        if self.closing || self.transfer.is_some() {
            return;
        }
        let Some(browser) = &self.browser else {
            self.status = "Refresh the folder to reconnect before starting a transfer".into();
            cx.notify();
            return;
        };
        let (events, progress) = tokio::sync::mpsc::channel(32);
        let _guard = self.runtime.enter();
        self.transfer = Some(browser.transfer(direction, events));
        self.progress = Some(progress);
        self.status = "Starting transfer…".into();
        cx.notify();
    }

    pub(in crate::ui) fn upload(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.transfer.is_some() {
            return;
        }
        let Some(directory) = &self.directory else {
            return;
        };
        let directory = directory.path.clone();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Upload a file".into()),
        });
        cx.spawn(async move |this, cx| match prompt.await {
            Ok(Ok(Some(paths))) => {
                if let Some(local) = paths.into_iter().next() {
                    let Some(name) = local.file_name().and_then(|name| name.to_str()) else {
                        return;
                    };
                    let remote = format!("{}/{name}", directory.trim_end_matches('/'));
                    let _ = this.update(cx, |this, cx| {
                        this.start_transfer(Direction::Upload { local, remote }, cx)
                    });
                }
            }
            Ok(Err(error)) => {
                let _ = this.update(cx, |this, cx| {
                    this.status = format!("Cannot choose upload file: {error}");
                    cx.notify();
                });
            }
            _ => (),
        })
        .detach();
    }

    pub(in crate::ui) fn download(&mut self, remote: String, name: &str, cx: &mut Context<Self>) {
        if self.busy || self.transfer.is_some() {
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
                let _ = this.update(cx, |this, cx| {
                    this.status = format!("Cannot choose download file: {error}");
                    cx.notify();
                });
            }
            _ => (),
        })
        .detach();
    }
}
