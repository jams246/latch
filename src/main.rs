#![windows_subsystem = "windows"]

mod ui;

use gpui::*;
use latch::storage::Store;
use std::{io, path::PathBuf, sync::Arc};

fn main() {
    let directory = match configuration_directory() {
        Ok(directory) => directory,
        Err(error) => {
            eprintln!("Cannot locate Latch settings: {error}");
            return;
        }
    };
    let roaming_directory = if std::env::var_os("LATCH_CONFIG_DIR").is_none() {
        std::env::var_os("APPDATA").map(|path| PathBuf::from(path).join("Latch"))
    } else {
        None
    };
    let store = match Store::load_portable(&directory, roaming_directory.as_deref()) {
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
        let mut options = ui::options(
            "Lightweight Access Terminal & Connection Handler",
            1120.,
            720.,
            cx,
        );
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

fn configuration_directory() -> io::Result<PathBuf> {
    if let Some(directory) = std::env::var_os("LATCH_CONFIG_DIR") {
        return Ok(PathBuf::from(directory));
    }
    std::env::current_exe()?
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| io::Error::other("The executable has no parent directory"))
}
