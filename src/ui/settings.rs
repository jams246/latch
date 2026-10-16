#[cfg(test)]
mod focus_tests;
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
}

impl Latch {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = self.settings.get_or_insert_with(|| Settings {
            focus: cx.focus_handle().tab_stop(false),
            preview: preview::new(cx),
            error: None,
        });
        window.focus(&settings.focus);
        cx.notify();
    }

    pub(super) fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings = None;
        let focus = self
            .active
            .and_then(|index| self.tabs.get(index))
            .map(|tab| tab.visible_focus(cx))
            .unwrap_or_else(|| self.connection_search.read(cx).focus.clone());
        window.focus(&focus);
        cx.notify();
    }

    fn choose_terminal_theme(&mut self, palette: TerminalPalette, cx: &mut Context<Self>) {
        let Some(settings) = &mut self.settings else {
            return;
        };
        match self.store.set_terminal_theme(palette) {
            Ok(()) => {
                settings.error = None;
                cx.set_global(palette);
                cx.refresh_windows();
            }
            Err(error) => settings.error = Some(format!("Cannot save terminal theme: {error}")),
        }
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
