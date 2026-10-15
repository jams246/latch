#![windows_subsystem = "windows"]

mod ui;

use gpui::*;
use latch::storage::Store;
use std::{path::PathBuf, sync::Arc};

fn main() {
    let directory = std::env::var_os("LATCH_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(|path| PathBuf::from(path).join("Latch")))
        .unwrap_or_else(|| PathBuf::from(".latch"));
    let store = match Store::load(directory.join("connections.json")) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("Cannot load Latch connections: {error}");
            return;
        }
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => Arc::new(runtime),
        Err(error) => {
            eprintln!("Cannot start network runtime: {error}");
            return;
        }
    };
    Application::new().run(move |cx: &mut App| {
        let mut options = ui::options("Latch", 1120., 720., cx);
        options.window_min_size = Some(size(px(720.), px(480.)));
        if let Err(error) = cx.open_window(options, |window, cx| {
            let owner = window.window_handle();
            let root = cx.new(|cx| ui::Latch::new(store, directory, runtime, owner, cx));
            let weak = root.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                let ready = weak
                    .update(cx, |app, cx| app.request_shutdown(cx))
                    .unwrap_or(true);
                if ready {
                    cx.quit();
                }
                ready
            });
            root
        }) {
            eprintln!("Failed to open window: {error}");
            cx.quit();
        }
    });
}
