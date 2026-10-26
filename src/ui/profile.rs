#[cfg(test)]
mod tests;

use super::{Latch, persistence::Saves};
use gpui::*;
use latch::storage::Store;
use std::{path::PathBuf, sync::Arc};

pub(super) struct Profile {
    pub(super) store: Store,
    pub(super) saves: Saves,
    pub(super) directory: PathBuf,
    pub(super) runtime: Arc<tokio::runtime::Runtime>,
    pub(super) windows: Vec<AnyWindowHandle>,
}

impl Profile {
    pub(super) fn new(
        store: Store,
        directory: PathBuf,
        runtime: Arc<tokio::runtime::Runtime>,
    ) -> Self {
        Self {
            store,
            directory,
            runtime,
            saves: Saves::default(),
            windows: Vec::new(),
        }
    }

    pub(super) fn window_count(&self, cx: &App) -> usize {
        let live = cx.windows();
        self.windows
            .iter()
            .filter(|window| live.contains(window))
            .count()
    }
}

// Keep profile ownership independent of any particular workspace window.
pub(super) struct ActiveProfile(pub(super) Entity<Profile>);
impl Global for ActiveProfile {}

impl Latch {
    pub(in crate::ui) fn store<'a>(&'a self, cx: &'a App) -> &'a Store {
        &self.profile.read(cx).store
    }
}

#[cfg(test)]
impl Latch {
    pub(in crate::ui) fn update_store<R>(
        &self,
        cx: &mut App,
        change: impl FnOnce(&mut Store) -> R,
    ) -> R {
        self.profile
            .update(cx, |profile, _| change(&mut profile.store))
    }
}
