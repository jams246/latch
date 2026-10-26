#![windows_subsystem = "windows"]

mod ui;

use gpui::*;
use latch::storage::Store;
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
    ui::windows::start(store, directory, runtime, cx)
}

fn executable_directory() -> io::Result<PathBuf> {
    std::env::current_exe()?
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| io::Error::other("The executable has no parent directory"))
}
