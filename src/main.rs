#![windows_subsystem = "windows"]

mod ui;

use gpui::*;
use latch::storage::{Store, WindowSize};
use std::{io, path::PathBuf, sync::Arc};

fn main() {
    let directory = match executable_directory() {
        Ok(directory) => directory,
        Err(error) => {
            eprintln!("Cannot locate Latch settings: {error}");
            return;
        }
    };
    let store = Store::load_profile(&directory);
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => Arc::new(runtime),
        Err(error) => {
            eprintln!("Cannot start network runtime: {error}");
            return;
        }
    };
    Application::new().run(move |cx: &mut App| {
        let result = match store {
            Ok(store) => open_latch(store, directory, runtime, cx),
            Err(error) => ui::recovery::open(error, directory, runtime, cx),
        };
        if let Err(error) = result {
            eprintln!("Failed to open Latch: {error}");
            cx.quit();
        }
    });
}

pub(crate) fn open_latch(
    store: Store,
    directory: PathBuf,
    runtime: Arc<tokio::runtime::Runtime>,
    cx: &mut App,
) -> anyhow::Result<()> {
    cx.set_global(store.terminal_theme());
    let window_size = store.window_size();
    let mut options = ui::options(
        "Lightweight Access Terminal & Connection Handler",
        window_size.width,
        window_size.height,
        cx,
    );
    options.window_min_size = Some(size(px(WindowSize::MIN_WIDTH), px(WindowSize::MIN_HEIGHT)));
    if window_size.maximized
        && let Some(bounds) = options.window_bounds
    {
        options.window_bounds = Some(WindowBounds::Maximized(bounds.get_bounds()));
    }
    cx.open_window(options, |window, cx| {
        let owner = window.window_handle();
        let root = cx.new(|cx| ui::Latch::new(store, directory, runtime, owner, cx));
        let weak = root.downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let bounds = window.window_bounds();
            let size = bounds.get_bounds().size;
            let ready = weak
                .update(cx, |app, cx| {
                    app.save_window_size(WindowSize {
                        width: f32::from(size.width).max(WindowSize::MIN_WIDTH),
                        height: f32::from(size.height).max(WindowSize::MIN_HEIGHT),
                        maximized: matches!(bounds, WindowBounds::Maximized(_)),
                    });
                    app.request_shutdown(cx)
                })
                .unwrap_or(true);
            if ready {
                cx.quit();
            }
            ready
        });
        root
    })?;
    Ok(())
}

fn executable_directory() -> io::Result<PathBuf> {
    std::env::current_exe()?
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| io::Error::other("The executable has no parent directory"))
}
