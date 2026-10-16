use super::*;
use latch::storage::{ImportSnapshot, Store};

impl Latch {
    fn begin_data(&mut self, cx: &mut Context<Self>) -> bool {
        if self.data_busy || self.shutting_down || self.settings.is_none() {
            return false;
        }
        self.data_busy = true;
        if let Some(settings) = &mut self.settings {
            settings.error = None;
            settings.message = None;
        }
        cx.notify();
        true
    }

    fn finish_data(&mut self, result: anyhow::Result<Option<&str>>, cx: &mut Context<Self>) {
        self.data_busy = false;
        if let Some(settings) = &mut self.settings {
            match result {
                Ok(message) => settings.message = message.map(str::to_owned),
                Err(error) => settings.error = Some(error.to_string()),
            }
        }
        cx.notify();
    }

    pub(super) fn export_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.begin_data(cx) {
            return;
        }
        let warning = window.prompt(
            PromptLevel::Warning,
            "Export unencrypted data?",
            Some("The JSON export includes connection details, settings and key-file paths. Anyone who can read the file can view or edit this data. Passwords and private keys are never included."),
            &["Cancel", "Export"],
            cx,
        );
        let runtime = self.runtime.clone();
        let directory = self.directory.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = async {
                if warning.await? != 1 {
                    return Ok(None);
                }
                let prompt = this.update(cx, |this, cx| {
                    cx.prompt_for_new_path(&this.directory, Some("latch-export.json"))
                })?;
                let Some(path) = prompt.await?? else {
                    return Ok(None);
                };
                let snapshot = this.update(cx, |this, _| this.store.snapshot())?;
                runtime
                    .spawn_blocking(move || {
                        let bytes = snapshot.export_json()?;
                        Store::write_export(&directory, &path, &bytes)
                    })
                    .await??;
                Ok(Some(
                    "Data exported. Keep the unencrypted file somewhere safe.",
                ))
            }
            .await;
            let _ = this.update(cx, |this, cx| this.finish_data(result, cx));
        })
        .detach();
    }

    pub(super) fn import_data(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_import() || !self.begin_data(cx) {
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose Latch JSON export".into()),
        });
        let runtime = self.runtime.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = async {
                let Some(paths) = prompt.await?? else {
                    return Ok(None);
                };
                let Some(path) = paths.into_iter().next() else {
                    return Ok(None);
                };
                let snapshot = runtime.spawn_blocking(move || Store::read_import_file(&path))
                    .await??;
                let confirmation = this.update_in(cx, |_, window, cx| {
                    window.prompt(
                        PromptLevel::Warning,
                        "Replace all saved data?",
                        Some("The file is valid. Import replaces all saved connections, settings, usage counts and saved folders. Nothing is merged or duplicated. Imported key-file paths may need updating on this computer."),
                        &["Cancel", "Replace all data"],
                        cx,
                    )
                })?;
                if confirmation.await? != 1 {
                    return Ok(None);
                }
                this.update(cx, |this, cx| this.apply_import(snapshot, cx))??;
                Ok(Some("Data imported. Saved window dimensions apply next time Latch starts."))
            }.await;
            let _ = this.update(cx, |this, cx| this.finish_data(result, cx));
        }).detach();
    }

    pub(super) fn apply_import(
        &mut self,
        snapshot: ImportSnapshot,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.tabs.is_empty()
                && self.pending_cleanup == 0
                && !self.shutting_down
                && self.editor.is_none()
                && self.connection_delete.is_none(),
            "Close all tabs and dialogs before importing data"
        );
        self.store.replace_import(snapshot)?;
        self.connection_search.update(cx, |input, cx| {
            input.replace(0..input.value.len(), "", cx);
        });
        self.preserve_imported_window = true;
        self.error = None;
        cx.set_global(self.store.terminal_theme());
        cx.refresh_windows();
        Ok(())
    }
}
