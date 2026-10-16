use super::{button, options, theme::CARBON};
use gpui::{prelude::*, *};
use latch::storage::Store;
use std::{io, path::PathBuf, sync::Arc};

#[cfg(test)]
mod tests;

pub(crate) fn open(
    error: io::Error,
    directory: PathBuf,
    runtime: Arc<tokio::runtime::Runtime>,
    cx: &mut App,
) -> anyhow::Result<()> {
    let allow_import = error.kind() != io::ErrorKind::WouldBlock;
    cx.open_window(
        options("Latch storage recovery", 680., 460., cx),
        |window, cx| {
            let root = cx.new(|cx| Recovery {
                directory,
                runtime,
                cause: error.to_string(),
                error: None,
                allow_import,
                busy: false,
                focus: cx.focus_handle(),
            });
            let weak = root.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                let ready = weak.update(cx, |this, _| !this.busy).unwrap_or(true);
                if ready {
                    cx.quit();
                }
                ready
            });
            window.focus(&root.read(cx).focus);
            root
        },
    )?;
    Ok(())
}

struct Recovery {
    directory: PathBuf,
    runtime: Arc<tokio::runtime::Runtime>,
    cause: String,
    error: Option<String>,
    allow_import: bool,
    busy: bool,
    focus: FocusHandle,
}

impl Recovery {
    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !self.allow_import {
            return;
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose Latch JSON export for recovery".into()),
        });
        let directory = self.directory.clone();
        let runtime = self.runtime.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result: anyhow::Result<Option<Store>> = async {
                let Some(paths) = prompt.await?? else {
                    return Ok(None);
                };
                let Some(path) = paths.into_iter().next() else {
                    return Ok(None);
                };
                let snapshot = runtime
                    .spawn_blocking(move || Store::read_import_file(&path))
                    .await??;
                let confirmation = this.update_in(cx, |_, window, cx| {
                    window.prompt(
                        PromptLevel::Warning,
                        "Recover Latch from this export?",
                        Some("The file is valid. Recovery creates a fresh encrypted store containing the exported connections, settings, usage counts and saved folders. Existing storage files are preserved. Imported key-file paths may need updating on this computer."),
                        &["Cancel", "Import and recover"],
                        cx,
                    )
                })?;
                if confirmation.await? != 1 {
                    return Ok(None);
                }
                runtime
                    .spawn_blocking(move || Store::recover_portable(&directory, snapshot))
                    .await?
                    .map(Some)
                    .map_err(Into::into)
            }
            .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(Some(store)) => {
                        match crate::open_latch(
                            store,
                            this.directory.clone(),
                            this.runtime.clone(),
                            cx,
                        ) {
                            Ok(()) => window.remove_window(),
                            Err(error) => this.error = Some(format!("Cannot open Latch: {error}")),
                        }
                    }
                    Ok(None) => (),
                    Err(error) => this.error = Some(format!("Cannot recover saved data: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for Recovery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("storage-recovery")
            .debug_selector(|| "storage-recovery".into())
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(rgb(CARBON.background))
            .text_color(rgb(CARBON.text))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev();
                    } else {
                        window.focus_next();
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .child(div().text_2xl().child("Cannot open saved data"))
            .child(div().text_color(rgb(CARBON.danger)).child(self.cause.clone()))
            .child(
                div().text_color(rgb(CARBON.muted)).child(
                    "Latch has not reset your data. Storage files may be damaged, unavailable or protected for a different Windows account or computer.",
                ),
            )
            .child(div().text_color(rgb(CARBON.muted)).child(if self.allow_import {
                "Import a previously exported JSON backup to recover, or exit and investigate the existing files."
            } else {
                "Another Latch process is using this profile. Close it before opening Latch again."
            }))
            .children(self.error.iter().map(|error| {
                div().text_color(rgb(CARBON.danger)).child(error.clone())
            }))
            .children(self.busy.then(|| div().child("Working...")))
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .gap_2()
                    .justify_end()
                    .children(self.allow_import.then(|| {
                        button("recovery-import", "Import backup")
                            .debug_selector(|| "recovery-import".into())
                            .when(self.busy, |button| button.opacity(0.5))
                            .on_click(cx.listener(|this, _, window, cx| this.import(window, cx)))
                    }))
                    .child(
                        button("recovery-exit", "Exit")
                            .debug_selector(|| "recovery-exit".into())
                            .when(self.busy, |button| button.opacity(0.5))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.busy {
                                    cx.quit();
                                }
                            })),
                    ),
            )
    }
}
