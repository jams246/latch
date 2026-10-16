mod auth_dialog;
mod connection_delete;
mod connections;
mod files;
mod files_view;
mod input;
mod input_handler;
mod input_paint;
#[cfg(test)]
mod input_tests;
mod lifecycle;
mod modal;
mod picker;
mod session_events;
mod sessions;
mod tabs;
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
    owner: AnyWindowHandle,
    cleanup_sender: std::sync::mpsc::Sender<lifecycle::Completion>,
    cleanup_events: std::sync::mpsc::Receiver<lifecycle::Completion>,
    pending_cleanup: usize,
    shutting_down: bool,
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
        let connection_search = cx.new(|cx| input::Input::new("", false, cx));
        cx.observe(&connection_search, |_, _, cx| cx.notify())
            .detach();
        Self {
            store,
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
            owner,
            cleanup_sender,
            cleanup_events,
            pending_cleanup: 0,
            shutting_down: false,
        }
    }
}

pub fn button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(0x263c4d))
        .text_color(rgb(0xe7f1fa))
        .cursor_pointer()
        .focusable()
        .tab_index(0)
        .focus(|style| style.bg(rgb(0x426c80)).border_color(rgb(0x9ce6d6)))
        .hover(|style| style.bg(rgb(0x36566a)))
        .child(text.into())
}

pub(super) fn dialog_header(title: impl Into<SharedString>) -> Div {
    div()
        .w_full()
        .flex_shrink_0()
        .px_4()
        .py_3()
        .rounded_t_lg()
        .bg(rgb(0x263c4d))
        .border_b_1()
        .border_color(rgb(0x426079))
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
