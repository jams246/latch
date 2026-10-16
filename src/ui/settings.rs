mod data;
#[cfg(test)]
mod data_tests;
mod data_view;
#[cfg(test)]
mod focus_tests;
#[cfg(test)]
mod guard_tests;
mod preview;
#[cfg(test)]
mod tests;
mod view;

use super::Latch;
use gpui::*;
use latch::terminal::{TerminalPalette, TerminalView};

pub(super) struct Settings {
    pub(super) focus: FocusHandle,
    pub(super) preview: Entity<TerminalView>,
    pub(super) error: Option<String>,
    page: Page,
    message: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Terminal,
    Data,
}

impl Latch {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.data_busy || self.shutting_down {
            return;
        }
        let settings = self.settings.get_or_insert_with(|| Settings {
            focus: cx.focus_handle().tab_stop(false),
            preview: preview::new(cx),
            error: None,
            page: Page::Terminal,
            message: None,
        });
        window.focus(&settings.focus);
        cx.notify();
    }

    pub(super) fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.data_busy {
            return;
        }
        self.settings = None;
        let focus = self
            .active
            .and_then(|index| self.tabs.get(index))
            .map(|tab| tab.visible_focus(cx))
            .unwrap_or_else(|| {
                if self.sidebar_collapsed {
                    self.sidebar_toggle_focus.clone()
                } else {
                    self.connection_search.read(cx).focus.clone()
                }
            });
        window.focus(&focus);
        cx.notify();
    }

    fn choose_terminal_theme(&mut self, palette: TerminalPalette, cx: &mut Context<Self>) {
        if self.data_busy || self.shutting_down {
            return;
        }
        if self.settings.is_none() {
            return;
        }
        self.save(
            move |store| store.set_terminal_theme(palette),
            move |this, result, _, cx| match result {
                Ok(()) => {
                    if let Some(settings) = &mut this.settings {
                        settings.error = None;
                    }
                    cx.set_global(palette);
                    cx.refresh_windows();
                }
                Err(error) => {
                    let error = Some(format!("Cannot save terminal theme: {error}"));
                    if let Some(settings) = &mut this.settings {
                        settings.error = error;
                    } else {
                        this.error = error;
                    }
                }
            },
            cx,
        );
        cx.notify();
    }

    fn settings_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "escape" => self.close_settings(window, cx),
            "tab" => {
                if event.keystroke.modifiers.shift {
                    window.focus_prev();
                } else {
                    window.focus_next();
                }
            }
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
    }
}
