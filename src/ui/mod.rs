mod auth_dialog;
mod connection_delete;
mod connections;
mod files;
mod files_view;
mod host_dialog;
mod input;
mod input_handler;
mod input_paint;
#[cfg(test)]
mod input_tests;
mod lifecycle;
mod modal;
mod persistence;
mod picker;
pub(crate) mod recovery;
mod session_events;
mod sessions;
mod settings;
mod sidebar;
mod tabs;
mod theme;
mod transport;
mod view;
mod workspace;

use gpui::*;
use latch::{
    ssh,
    storage::{Connection, Store},
    terminal::TerminalView,
};
use std::{path::PathBuf, sync::Arc};

pub struct Latch {
    store: Store,
    saves: persistence::Saves,
    connection_search: Entity<input::Input>,
    directory: PathBuf,
    runtime: Arc<tokio::runtime::Runtime>,
    tabs: Vec<Tab>,
    active: Option<usize>,
    tab_scroll: ScrollHandle,
    tab_drag: tabs::TabDrag,
    tab_reveal_key: Option<(String, usize, Pixels)>,
    error: Option<String>,
    editor: Option<connections::Editor>,
    connection_delete: Option<connection_delete::DeleteDialog>,
    settings: Option<settings::Settings>,
    owner: AnyWindowHandle,
    cleanup_sender: std::sync::mpsc::Sender<lifecycle::Completion>,
    cleanup_events: std::sync::mpsc::Receiver<lifecycle::Completion>,
    pending_cleanup: usize,
    shutting_down: bool,
    data_busy: bool,
    preserve_imported_window: bool,
    window_save_failed: bool,
    preference_sender: std::sync::mpsc::Sender<LocalPreference>,
    preference_events: std::sync::mpsc::Receiver<LocalPreference>,
}

struct LocalPreference {
    tab_id: String,
    connection_id: String,
    path: PathBuf,
}

struct Tab {
    id: String,
    connection: Connection,
    name: String,
    destination: String,
    state: ssh::State,
    error: Option<String>,
    counted: bool,
    transport: Option<transport::Transport>,
    content: TabContent,
    dialog: Option<WindowHandle<auth_dialog::AuthDialog>>,
    host_dialog: Option<host_dialog::HostDialog>,
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
    pub fn new(
        store: Store,
        directory: PathBuf,
        runtime: Arc<tokio::runtime::Runtime>,
        owner: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> Self {
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
        cx.observe(&connection_search, |_, _, cx| cx.notify())
            .detach();
        Self {
            store,
            saves: persistence::Saves::default(),
            connection_search,
            directory,
            runtime,
            tabs: Vec::new(),
            active: None,
            tab_scroll: ScrollHandle::new(),
            tab_drag: tabs::TabDrag::default(),
            tab_reveal_key: None,
            error: None,
            editor: None,
            connection_delete: None,
            settings: None,
            owner,
            cleanup_sender,
            cleanup_events,
            pending_cleanup: 0,
            shutting_down: false,
            data_busy: false,
            preserve_imported_window: false,
            window_save_failed: false,
            preference_sender,
            preference_events,
        }
    }

    fn can_import(&self) -> bool {
        self.tabs.is_empty()
            && self.pending_cleanup == 0
            && !self.shutting_down
            && !self.data_busy
            && !self.saves_pending()
            && self.editor.is_none()
            && self.connection_delete.is_none()
    }
}

pub fn button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    let colors = theme::CARBON;
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(colors.control))
        .border_1()
        .border_color(rgb(colors.border))
        .text_color(rgb(colors.text))
        .cursor_pointer()
        .focusable()
        .tab_index(0)
        .focus(move |style| style.border_color(rgb(colors.accent)))
        .hover(move |style| style.bg(rgb(colors.hover)))
        .child(text.into())
}

pub(super) fn dialog_header(title: impl Into<SharedString>) -> Div {
    let colors = theme::CARBON;
    div()
        .w_full()
        .flex_shrink_0()
        .px_4()
        .py_3()
        .rounded_t_lg()
        .bg(rgb(colors.control))
        .border_b_1()
        .border_color(rgb(colors.border))
        .text_lg()
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.into())
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
