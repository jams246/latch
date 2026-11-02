mod auth_dialog;
mod choices;
mod connection_delete;
mod connections;
mod controls;
pub use controls::button;
use controls::dialog_header;
mod files;
mod files_view;
mod host_dialog;
mod input;
mod input_handler;
mod input_paint;
#[cfg(test)]
mod input_tests;
mod library;
mod lifecycle;
mod modal;
mod persistence;
mod picker;
mod profile;
mod reconnect;
pub(crate) mod recovery;
mod session_events;
mod sessions;
mod settings;
mod tabs;
mod theme;
mod transport;
mod view;
pub(crate) mod windows;
#[cfg(test)]
mod windows_tests;
mod workspace;

use gpui::*;
use latch::{ssh, storage::Connection, terminal::TerminalView};
use std::{path::PathBuf, sync::Arc};

pub struct Latch {
    profile: Entity<profile::Profile>,
    pending_saves: usize,
    connection_search: Entity<input::Input>,
    directory: PathBuf,
    runtime: Arc<tokio::runtime::Runtime>,
    tabs: Vec<Tab>,
    active: Option<usize>,
    tab_scroll: ScrollHandle,
    connection_scroll: ScrollHandle,
    connection_scroll_drag: Option<Pixels>,
    tab_drag: tabs::TabDrag,
    tab_menu: Option<tabs::menu::TabMenu>,
    tab_reveal_key: Option<(String, usize, Pixels)>,
    tab_bounds: Option<Bounds<Pixels>>,
    drop_highlight: bool,
    error: Option<String>,
    editor: Option<connections::Editor>,
    connection_delete: Option<connection_delete::DeleteDialog>,
    settings: Option<settings::Settings>,
    popup: Option<choices::Popup>,
    owner: AnyWindowHandle,
    cleanup_sender: std::sync::mpsc::Sender<lifecycle::Completion>,
    cleanup_events: std::sync::mpsc::Receiver<lifecycle::Completion>,
    pending_cleanup: usize,
    shutting_down: bool,
    data_busy: bool,
    preserve_imported_window: bool,
    window_save_failed: bool,
    closing_size: Option<latch::storage::WindowSize>,
    preference_sender: std::sync::mpsc::Sender<LocalPreference>,
    preference_events: std::sync::mpsc::Receiver<LocalPreference>,
}

struct LocalPreference {
    tab_id: String,
    connection_id: String,
    path: PathBuf,
}

struct Tab {
    header_focus: FocusHandle,
    id: String,
    connection: Connection,
    name: String,
    destination: String,
    state: ssh::State,
    error: Option<String>,
    counted: bool,
    save_password: bool,
    password_attempt: Option<ssh::Secret>,
    transport: Option<transport::Transport>,
    content: TabContent,
    dialog: Option<WindowHandle<auth_dialog::AuthDialog>>,
    host_dialog: Option<host_dialog::HostDialog>,
    reconnect: Option<reconnect::ReconnectDialog>,
    events: std::sync::mpsc::Receiver<ssh::Event>,
    prompts: tokio::sync::mpsc::Receiver<ssh::Prompt>,
    closing: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SessionKind {
    Terminal,
    Sftp,
}

enum TabContent {
    Terminal(Entity<TerminalView>),
    Sftp {
        files: Option<Entity<files::Files>>,
        focus: FocusHandle,
    },
}

impl Latch {
    #[cfg(test)]
    pub fn new(
        store: latch::storage::Store,
        directory: PathBuf,
        runtime: Arc<tokio::runtime::Runtime>,
        owner: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let profile = cx.new(|_| profile::Profile::new(store, directory, runtime));
        Self::from_profile(profile, owner, cx)
    }

    fn from_profile(
        profile: Entity<profile::Profile>,
        owner: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        theme::apply(profile.read(cx).store.app_theme(), cx.window_appearance());
        let directory = profile.read(cx).directory.clone();
        let runtime = profile.read(cx).runtime.clone();
        profile.update(cx, |profile, cx| {
            profile.windows.push(owner);
            cx.notify();
        });
        cx.observe(&profile, |_, _, cx| cx.notify()).detach();
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(25)).await;
                if this.update(cx, |this, cx| this.poll(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let (cleanup_sender, cleanup_events) = std::sync::mpsc::channel();
        let (preference_sender, preference_events) = std::sync::mpsc::channel();
        let connection_search = cx.new(|cx| input::Input::new("", false, cx));
        let mut last_query = String::new();
        cx.observe(&connection_search, move |this, input, cx| {
            let query = input.read(cx).value.as_str();
            if last_query != query {
                last_query.clear();
                last_query.push_str(query);
                this.connection_scroll.set_offset(Point::default());
            }
            cx.notify();
        })
        .detach();
        Self {
            profile,
            pending_saves: 0,
            connection_search,
            directory,
            runtime,
            tabs: Vec::new(),
            active: None,
            tab_scroll: ScrollHandle::new(),
            connection_scroll: ScrollHandle::new(),
            connection_scroll_drag: None,
            tab_drag: tabs::TabDrag::default(),
            tab_menu: None,
            tab_reveal_key: None,
            tab_bounds: None,
            drop_highlight: false,
            error: None,
            editor: None,
            connection_delete: None,
            settings: None,
            popup: None,
            owner,
            cleanup_sender,
            cleanup_events,
            pending_cleanup: 0,
            shutting_down: false,
            data_busy: false,
            preserve_imported_window: false,
            window_save_failed: false,
            closing_size: None,
            preference_sender,
            preference_events,
        }
    }

    fn can_import(&self, cx: &App) -> bool {
        self.profile.read(cx).window_count(cx) == 1
            && !self.profile.read(cx).saves_pending()
            && self.tabs.is_empty()
            && self.pending_cleanup == 0
            && !self.shutting_down
            && !self.data_busy
            && !self.saves_pending()
            && self.editor.is_none()
            && self.connection_delete.is_none()
    }
}

pub fn options(title: &str, width: f32, height: f32, cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(width), px(height)), cx)),
        titlebar: Some(TitlebarOptions {
            title: Some(title.to_string().into()),
            ..Default::default()
        }),
        window_min_size: Some(size(px(420.), px(240.))),
        ..Default::default()
    }
}
