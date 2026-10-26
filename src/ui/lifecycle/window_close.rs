#[cfg(test)]
mod tests;

use crate::ui::Latch;
use gpui::*;

impl Latch {
    pub(in crate::ui) fn finish_window_close(&mut self, cx: &mut Context<Self>) {
        self.shutting_down = true;
        if let Some(size) = self.closing_size.take()
            && !self.preserve_imported_window
            && !self.window_save_failed
        {
            self.save(move |store| store.set_window_size(size), |this, result, window, _| {
                match result {
                    Ok(()) => window.remove_window(),
                    Err(error) => {
                        this.window_save_failed = true;
                        this.save_error(format!("Cannot save window size: {error}. Close again to exit without saving the window size."));
                    }
                }
            }, cx);
        } else {
            let _ = self.owner.update(cx, |_, window, _| window.remove_window());
        }
    }
}
