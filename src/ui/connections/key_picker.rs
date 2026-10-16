use super::*;
use std::path::PathBuf;

impl Latch {
    pub(super) fn browse_key(&mut self, cx: &mut Context<Self>) {
        if self.data_busy {
            return;
        }
        let Some(editor) = &self.editor else {
            return;
        };
        let field = editor.fields[4].entity_id();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose private-key file".into()),
        });
        cx.spawn(async move |this, cx| {
            let result = match prompt.await {
                Ok(result) => result,
                Err(error) => Err(error.into()),
            };
            let _ = this.update(cx, |this, cx| this.apply_key_selection(field, result, cx));
        })
        .detach();
    }

    pub(super) fn apply_key_selection(
        &mut self,
        field: EntityId,
        result: anyhow::Result<Option<Vec<PathBuf>>>,
        cx: &mut Context<Self>,
    ) {
        if self.data_busy {
            return;
        }
        let Some(editor) = &self.editor else {
            return;
        };
        if editor.fields[4].entity_id() != field {
            return;
        }
        match result {
            Ok(Some(paths)) => {
                let Some(path) = paths.first() else {
                    return;
                };
                let Some(path) = path.to_str() else {
                    self.error =
                        Some("Private-key file path contains unsupported characters".into());
                    cx.notify();
                    return;
                };
                editor.fields[4].update(cx, |input, cx| {
                    input.replace(0..input.value.len(), path, cx);
                });
                self.error = None;
                cx.notify();
            }
            Err(error) => {
                self.error = Some(format!("Cannot choose private-key file: {error}"));
                cx.notify();
            }
            Ok(None) => {}
        }
    }
}
