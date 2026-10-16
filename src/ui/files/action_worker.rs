use super::{
    actions::{ActionDialog, ActionKind, ActionResult, Target},
    *,
};

impl Files {
    pub(super) fn start_action(
        &mut self,
        dialog: ActionDialog,
        name: String,
        cx: &mut Context<Self>,
    ) {
        let side = dialog.side;
        let kind = dialog.kind;
        let browser = self.browser.clone();
        let commands = self.commands.clone();
        let sender = self.action_sender.clone();
        self.action_busy = true;
        self.action_side = Some(side);
        self.action_task = Some(self.runtime.spawn(async move {
            let mut connection_lost = false;
            let result = match dialog.parent {
                Target::Local(parent) => tokio::task::spawn_blocking(move || match kind {
                    ActionKind::Rename => match dialog.targets.first() {
                        Some(Target::Local(path)) => local::rename(path, &name),
                        _ => Err(std::io::Error::other("Select one local item to rename")),
                    },
                    ActionKind::NewFolder => local::create_directory(&parent, &name),
                    ActionKind::Delete => {
                        let paths = dialog
                            .targets
                            .into_iter()
                            .map(|target| match target {
                                Target::Local(path) => Ok(path),
                                _ => Err(std::io::Error::other("Invalid local selection")),
                            })
                            .collect::<std::io::Result<Vec<_>>>()?;
                        local::delete(&paths)
                    }
                    ActionKind::Copy | ActionKind::Refresh => {
                        Err(std::io::Error::other("Invalid file action"))
                    }
                })
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result.map_err(|error| error.to_string())),
                Target::Remote(parent) => {
                    async {
                        let browser = match browser {
                            Some(browser) => browser,
                            None => Browser::connect(&commands).await.map_err(|error| {
                                connection_lost = recovery::connection_error(&error);
                                error.to_string()
                            })?,
                        };
                        match kind {
                            ActionKind::Rename => match dialog.targets.first() {
                                Some(Target::Remote(path)) => browser.rename(path, &name).await,
                                _ => return Err("Select one remote item to rename".into()),
                            },
                            ActionKind::NewFolder => browser.create_directory(&parent, &name).await,
                            ActionKind::Delete => {
                                let paths = dialog
                                    .targets
                                    .into_iter()
                                    .map(|target| match target {
                                        Target::Remote(path) => Ok(path),
                                        _ => Err("Invalid remote selection".to_string()),
                                    })
                                    .collect::<Result<Vec<_>, _>>()?;
                                browser.delete(&paths).await
                            }
                            ActionKind::Copy | ActionKind::Refresh => {
                                return Err("Invalid file action".into());
                            }
                        }
                        .map_err(|error| {
                            connection_lost = recovery::connection_error(&error);
                            error.to_string()
                        })
                    }
                    .await
                }
            };
            let result = result.map_err(|error| {
                if kind == ActionKind::Delete {
                    format!("{error} Some selected items may already have been deleted.")
                } else {
                    error
                }
            });
            let _ = sender.send(ActionResult {
                side,
                result,
                connection_lost,
            });
        }));
        cx.notify();
    }

    pub(super) fn poll_actions(&mut self, cx: &mut Context<Self>) {
        while let Ok(event) = self.action_events.try_recv() {
            self.action_busy = false;
            self.action_side = None;
            self.action_task = None;
            if let Err(error) = event.result {
                if event.side == Side::Remote {
                    self.browser = None;
                }
                self.error = Some(format!("File action failed: {error}"));
            }
            if event.connection_lost {
                self.request_reconnect(cx);
            }
            self.refresh(event.side, cx);
            cx.notify();
        }
    }
}
