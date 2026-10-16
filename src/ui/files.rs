mod action_dialog;
mod action_menu;
mod action_worker;
mod actions;
pub(super) mod breadcrumbs;
pub(super) mod conflicts;
mod copy_actions;
pub(super) mod local;
mod navigation;
pub(super) mod operations;
pub(super) mod pane;
mod polling;
pub(super) mod rows;
pub(super) mod selection;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_actions;
#[cfg(test)]
mod tests_interaction;
#[cfg(test)]
mod tests_navigation;
#[cfg(test)]
mod tests_refresh;
mod transfers;

use super::input::Input;
use gpui::*;
use latch::{
    sftp::{self, Browser, Direction, Directory, TransferEvent},
    ssh, storage,
};
use pane::{History, Side, Sort};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
};

enum RemoteResult {
    Directory(Browser, Directory),
    FileLink(Browser, String),
}

type LocalResult = std::io::Result<(local::Directory, Option<String>)>;

pub(super) struct Files {
    pub(super) connection_name: String,
    connection_id: String,
    pub path: Entity<Input>,
    pub directory: Option<Directory>,
    pub local: Option<local::Directory>,
    pub status: String,
    pub local_status: String,
    pub busy: bool,
    pub local_busy: bool,
    pub local_sort: Sort,
    pub remote_sort: Sort,
    pub local_history: History<PathBuf>,
    pub remote_history: History<String>,
    selections: [selection::Selection; 2],
    menu: Option<actions::ContextMenu>,
    dialog: Option<actions::ActionDialog>,
    action_events: mpsc::Receiver<actions::ActionResult>,
    action_sender: mpsc::Sender<actions::ActionResult>,
    action_task: Option<tokio::task::JoinHandle<()>>,
    action_busy: bool,
    batch_copy: bool,
    pub(super) action_status: [Option<String>; 2],
    local_breadcrumb_scroll: ScrollHandle,
    remote_breadcrumb_scroll: ScrollHandle,
    pub(super) breadcrumb_reveal: [bool; 2],
    pub transfer: Option<sftp::Transfer>,
    metrics: Option<operations::Metrics>,
    conflict: Option<conflicts::Dialog>,
    browser: Option<Browser>,
    runtime: Arc<tokio::runtime::Runtime>,
    events: mpsc::Receiver<Result<RemoteResult, sftp::Error>>,
    sender: mpsc::Sender<Result<RemoteResult, sftp::Error>>,
    local_events: mpsc::Receiver<LocalResult>,
    local_sender: mpsc::Sender<LocalResult>,
    progress: Option<tokio::sync::mpsc::Receiver<TransferEvent>>,
    operation: Option<tokio::task::JoinHandle<()>>,
    local_operation: Option<tokio::task::JoinHandle<()>>,
    commands: tokio::sync::mpsc::Sender<ssh::Command>,
    config_directory: PathBuf,
    pending_local_history: Option<usize>,
    pending_remote_history: Option<usize>,
    closing: bool,
}

impl Drop for Files {
    fn drop(&mut self) {
        for task in [&self.operation, &self.local_operation, &self.action_task]
            .into_iter()
            .flatten()
        {
            task.abort();
        }
    }
}

impl Files {
    pub fn new(
        connection: &storage::Connection,
        commands: tokio::sync::mpsc::Sender<ssh::Command>,
        runtime: Arc<tokio::runtime::Runtime>,
        config_directory: PathBuf,
        cx: &mut Context<Self>,
    ) -> Self {
        let (sender, events) = mpsc::channel();
        let (local_sender, local_events) = mpsc::channel();
        let (action_sender, action_events) = mpsc::channel();
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(30)).await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let mut files = Self {
            connection_name: connection.name.clone(),
            connection_id: connection.id.clone(),
            path: cx.new(|cx| Input::new(".", false, cx)),
            directory: None,
            local: None,
            status: "Opening SFTP…".into(),
            local_status: "Loading local directory…".into(),
            busy: false,
            local_busy: false,
            local_sort: Sort::default(),
            remote_sort: Sort::default(),
            local_history: History::default(),
            remote_history: History::default(),
            selections: Default::default(),
            menu: None,
            dialog: None,
            action_events,
            action_sender,
            action_task: None,
            action_busy: false,
            batch_copy: false,
            action_status: Default::default(),
            local_breadcrumb_scroll: ScrollHandle::new(),
            remote_breadcrumb_scroll: ScrollHandle::new(),
            breadcrumb_reveal: [false; 2],
            transfer: None,
            metrics: None,
            conflict: None,
            browser: None,
            runtime,
            events,
            sender,
            local_events,
            local_sender,
            progress: None,
            operation: None,
            local_operation: None,
            commands,
            config_directory,
            pending_local_history: None,
            pending_remote_history: None,
            closing: false,
        };
        files.navigate(".".into(), cx);
        files.load_local(None, false, None, cx);
        files
    }

    pub(super) fn prepare_close(&mut self, cx: &mut Context<Self>) -> Option<sftp::Transfer> {
        self.closing = true;
        self.busy = true;
        self.menu = None;
        self.dialog = None;
        for operation in [
            self.operation.take(),
            self.local_operation.take(),
            self.action_task.take(),
        ]
        .into_iter()
        .flatten()
        {
            operation.abort();
        }
        self.progress = None;
        self.metrics = None;
        self.conflict = None;
        self.browser = None;
        cx.notify();
        self.transfer.take()
    }
}
