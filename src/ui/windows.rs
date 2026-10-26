use super::{
    Latch, SessionKind,
    profile::{ActiveProfile, Profile},
};
use gpui::*;
use latch::storage::{Connection, Store, WindowSize};
use std::{path::PathBuf, sync::Arc};

pub(crate) fn start(
    store: Store,
    directory: PathBuf,
    runtime: Arc<tokio::runtime::Runtime>,
    cx: &mut App,
) -> anyhow::Result<()> {
    cx.set_global(store.terminal_theme());
    let profile = cx.new(|_| Profile::new(store, directory, runtime));
    cx.set_global(ActiveProfile(profile.clone()));
    cx.on_window_closed(|cx| {
        let profile = cx.global::<ActiveProfile>().0.clone();
        let live = cx.windows();
        profile.update(cx, |profile, cx| {
            profile.windows.retain(|handle| live.contains(handle));
            cx.notify();
            if profile.windows.is_empty() {
                cx.quit();
            }
        });
    })
    .detach();
    open(profile, None, None, cx)?;
    Ok(())
}

fn open(
    profile: Entity<Profile>,
    source: Option<&Window>,
    connection: Option<Connection>,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<Latch>> {
    let saved = profile.read(cx).store.window_size();
    let mut options = super::options(
        "Latch - Lightweight Access Terminal & Connection Handler",
        saved.width,
        saved.height,
        cx,
    );
    options.window_min_size = Some(size(px(WindowSize::MIN_WIDTH), px(WindowSize::MIN_HEIGHT)));
    if let Some(source) = source {
        options.window_bounds = Some(WindowBounds::Windowed(super::tabs::native::placement(
            source, saved,
        )?));
    } else if saved.maximized
        && let Some(bounds) = options.window_bounds
    {
        options.window_bounds = Some(WindowBounds::Maximized(bounds.get_bounds()));
    }
    let handle = cx.open_window(options, |window, cx| {
        let owner = window.window_handle();
        let root = cx.new(|cx| Latch::from_profile(profile, owner, cx));
        let weak = root.downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let bounds = window.window_bounds();
            let size = bounds.get_bounds().size;
            weak.update(cx, |app, cx| {
                if !app.shutting_down {
                    app.closing_size = Some(WindowSize {
                        width: f32::from(size.width).max(WindowSize::MIN_WIDTH),
                        height: f32::from(size.height).max(WindowSize::MIN_HEIGHT),
                        maximized: matches!(bounds, WindowBounds::Maximized(_)),
                    });
                }
                if app.request_shutdown(cx) {
                    if app.preserve_imported_window || app.window_save_failed {
                        return true;
                    }
                    app.finish_window_close(cx);
                }
                false
            })
            .unwrap_or(true)
        });
        if let Some(connection) = connection {
            root.update(cx, |app, cx| {
                app.connect(connection, SessionKind::Terminal, window, cx)
            });
        }
        root
    })?;
    handle.update(cx, |_, window, _| window.activate_window())?;
    Ok(handle)
}

impl Latch {
    pub(super) fn duplicate_to_window(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shutting_down || self.data_busy || self.settings.is_some() {
            return;
        }
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        if tab.closing || !matches!(tab.content, super::TabContent::Terminal(_)) {
            return;
        }
        let connection = tab.connection.clone();
        if let Err(error) = open(self.profile.clone(), Some(window), Some(connection), cx) {
            self.error = Some(format!("Cannot open new window: {error}"));
            cx.notify();
        }
    }
}
