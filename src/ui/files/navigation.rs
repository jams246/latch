use super::*;

impl Files {
    pub(in crate::ui) fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        self.load_remote(path, false, None, cx);
    }

    fn load_remote(
        &mut self,
        path: String,
        follow_link: bool,
        history: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if self.busy
            || self.closing
            || self.action_busy
            || self.dialog.is_some()
            || self.transfer.is_some()
        {
            return;
        }
        let browser = self.browser.clone();
        let commands = self.commands.clone();
        let events = self.sender.clone();
        self.busy = true;
        self.menu = None;
        self.action_status[1] = None;
        self.pending_remote_history = history;
        self.status = "Loading directory…".into();
        self.operation = Some(self.runtime.spawn(async move {
            let result = async {
                let browser = match browser {
                    Some(browser) => browser,
                    None => Browser::connect(&commands).await?,
                };
                let path = if follow_link {
                    match browser.resolve_link(&path).await? {
                        Some(path) => path,
                        None => return Ok(RemoteResult::FileLink(browser, path)),
                    }
                } else {
                    path
                };
                let directory = browser.list(&path).await?;
                Ok(RemoteResult::Directory(browser, directory))
            }
            .await;
            let _ = events.send(result);
        }));
        cx.notify();
    }

    pub(in crate::ui) fn navigate_local(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.load_local(Some(path), false, None, cx);
    }

    pub(super) fn load_local(
        &mut self,
        path: Option<PathBuf>,
        follow_link: bool,
        history: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if self.local_busy
            || self.closing
            || self.action_busy
            || self.dialog.is_some()
            || self.transfer.is_some()
        {
            return;
        }
        let config_directory = self.config_directory.clone();
        let connection_id = self.connection_id.clone();
        let events = self.local_sender.clone();
        self.local_busy = true;
        self.menu = None;
        self.action_status[0] = None;
        self.pending_local_history = history;
        self.local_status = "Loading directory…".into();
        self.local_operation = Some(self.runtime.spawn_blocking(move || {
            let result = (|| {
                let directory = match path {
                    Some(path) => {
                        let path = if follow_link {
                            local::follow_link(&path)?
                        } else {
                            path
                        };
                        local::list(&path)?
                    }
                    None => local::list_initial(&config_directory, &connection_id)?,
                };
                let warning = local::remember(&config_directory, &connection_id, &directory.path)
                    .err()
                    .map(|error| format!("Cannot remember local directory: {error}"));
                Ok((directory, warning))
            })();
            let _ = events.send(result);
        }));
        cx.notify();
    }

    pub(in crate::ui) fn history_back(&mut self, side: Side, cx: &mut Context<Self>) {
        match side {
            Side::Local => {
                if let Some((index, path)) = self.local_history.target_back() {
                    self.load_local(Some(path), false, Some(index), cx);
                }
            }
            Side::Remote => {
                if let Some((index, path)) = self.remote_history.target_back() {
                    self.load_remote(path, false, Some(index), cx);
                }
            }
        }
    }

    pub(in crate::ui) fn history_forward(&mut self, side: Side, cx: &mut Context<Self>) {
        match side {
            Side::Local => {
                if let Some((index, path)) = self.local_history.target_forward() {
                    self.load_local(Some(path), false, Some(index), cx);
                }
            }
            Side::Remote => {
                if let Some((index, path)) = self.remote_history.target_forward() {
                    self.load_remote(path, false, Some(index), cx);
                }
            }
        }
    }

    pub(in crate::ui) fn activate(&mut self, side: Side, index: usize, cx: &mut Context<Self>) {
        match side {
            Side::Local => {
                let Some(entry) = self.local.as_ref().and_then(|d| d.entries.get(index)) else {
                    return;
                };
                if entry.metadata.is_dir || entry.metadata.is_link {
                    self.load_local(Some(entry.path.clone()), entry.metadata.is_link, None, cx);
                }
            }
            Side::Remote => {
                let Some(entry) = self.directory.as_ref().and_then(|d| d.entries.get(index)) else {
                    return;
                };
                if entry.is_link || entry.is_dir {
                    self.load_remote(entry.path.clone(), entry.is_link, None, cx);
                } else {
                    let path = entry.path.clone();
                    let name = entry.name.clone();
                    self.download(path, &name, cx);
                }
            }
        }
    }

    pub(in crate::ui) fn refresh(&mut self, side: Side, cx: &mut Context<Self>) {
        match side {
            Side::Local => {
                if let Some(directory) = &self.local {
                    self.navigate_local(directory.path.clone(), cx);
                } else {
                    self.load_local(None, false, None, cx);
                }
            }
            Side::Remote => {
                let path = self.directory.as_ref().map_or(".", |d| d.path.as_str());
                self.navigate(path.to_string(), cx);
            }
        }
    }

    pub(in crate::ui) fn navigate_parent(&mut self, side: Side, cx: &mut Context<Self>) {
        match side {
            Side::Local => {
                if let Some(path) = self
                    .local
                    .as_ref()
                    .and_then(|directory| directory.path.parent())
                {
                    self.navigate_local(path.to_path_buf(), cx);
                }
            }
            Side::Remote => {
                if let Some(directory) = &self.directory {
                    self.navigate(format!("{}/..", directory.path.trim_end_matches('/')), cx);
                }
            }
        }
    }
}
