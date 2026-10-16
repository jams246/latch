use super::Latch;
use gpui::*;
use latch::storage::Store;
use std::{collections::VecDeque, io};

#[cfg(test)]
pub(super) mod tests;

type Change = Box<dyn FnOnce(&mut Store) -> io::Result<()> + Send>;
type Finish = Box<dyn FnOnce(&mut Latch, io::Result<()>, &mut Window, &mut Context<Latch>)>;

#[derive(Default)]
pub(super) struct Saves {
    pending: VecDeque<(Change, Finish)>,
    active: bool,
}

impl Latch {
    pub(super) fn save_error(&mut self, error: String) {
        if let Some(settings) = &mut self.settings {
            settings.error = Some(error.clone());
        }
        self.error = Some(error);
    }

    pub(super) fn saves_pending(&self) -> bool {
        self.saves.active || !self.saves.pending.is_empty()
    }

    pub(super) fn save(
        &mut self,
        change: impl FnOnce(&mut Store) -> io::Result<()> + Send + 'static,
        finish: impl FnOnce(&mut Self, io::Result<()>, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.saves
            .pending
            .push_back((Box::new(change), Box::new(finish)));
        self.start_next_save(cx);
    }

    pub(super) fn save_preference(
        &mut self,
        label: &'static str,
        change: impl FnOnce(&mut Store) -> io::Result<()> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        self.save(
            change,
            move |this, result, _, _| {
                if let Err(error) = result {
                    this.save_error(format!("Cannot save {label}: {error}"));
                }
            },
            cx,
        );
    }

    fn start_next_save(&mut self, cx: &mut Context<Self>) {
        if self.saves.active {
            return;
        }
        let Some((change, finish)) = self.saves.pending.pop_front() else {
            return;
        };
        self.saves.active = true;
        // Each mutation starts from the last committed state, including earlier queued edits.
        let mut store = self.store.clone();
        let task = self.runtime.spawn_blocking(move || {
            change(&mut store)?;
            Ok::<_, io::Error>(store)
        });
        let owner = self.owner;
        cx.spawn(async move |this, cx| {
            let result = task
                .await
                .map_err(io::Error::other)
                .and_then(|result| result);
            let _ = owner.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    this.saves.active = false;
                    let result = match result {
                        Ok(store) => {
                            this.store = store;
                            Ok(())
                        }
                        Err(error) => {
                            this.shutting_down = false;
                            Err(error)
                        }
                    };
                    finish(this, result, window, cx);
                    this.start_next_save(cx);
                    cx.notify();
                })
            });
        })
        .detach();
    }
}
