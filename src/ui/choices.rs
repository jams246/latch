use super::{Latch, controls::quiet, theme::current};
use gpui::{prelude::*, *};
use latch::storage::{SortMode, TerminalTheme};

pub(super) const SORTS: [SortMode; 3] = [
    SortMode::Name,
    SortMode::MostUsed,
    SortMode::FrequentAndName,
];

pub(super) enum Choice {
    Sort(SortMode),
    Terminal(TerminalTheme),
    Edit(String),
    Delete(String),
}

impl Choice {
    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::Sort(SortMode::Name) => "Name",
            Self::Sort(SortMode::MostUsed) => "Most used",
            Self::Sort(SortMode::FrequentAndName) => "Top 3 + name",
            Self::Terminal(theme) => theme.name(),
            Self::Edit(_) => "Edit connection",
            Self::Delete(_) => "Delete connection",
        }
    }
}

pub(super) struct Popup {
    items: Vec<(Choice, FocusHandle)>,
    position: Point<Pixels>,
    previous: Option<FocusHandle>,
}

impl Latch {
    pub(super) fn show_choices(
        &mut self,
        choices: impl IntoIterator<Item = Choice>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.data_busy || self.shutting_down {
            return;
        }
        let items: Vec<_> = choices
            .into_iter()
            .map(|choice| (choice, cx.focus_handle().tab_stop(true)))
            .collect();
        let previous = window.focused(cx);
        let Some((_, focus)) = items.first() else {
            return;
        };
        window.focus(focus);
        self.popup = Some(Popup {
            items,
            position,
            previous,
        });
        cx.notify();
    }

    pub(super) fn dismiss_choices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.popup.take() {
            if let Some(previous) = menu.previous {
                window.focus(&previous)
            }
            cx.notify();
        }
    }

    fn pick_choice(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.popup.take() else {
            return;
        };
        if let Some(previous) = menu.previous {
            window.focus(&previous)
        }
        let Some((choice, _)) = menu.items.into_iter().nth(index) else {
            return;
        };
        if self.data_busy || self.shutting_down {
            cx.notify();
            return;
        }
        let delete = matches!(choice, Choice::Delete(_));
        match choice {
            Choice::Sort(mode) => {
                self.connection_scroll.set_offset(Point::default());
                self.save_preference("sorting", move |store| store.set_sort_mode(mode), cx)
            }
            Choice::Terminal(theme) => self.choose_terminal_theme(theme, cx),
            Choice::Edit(id) | Choice::Delete(id) => {
                if let Some(conn) = self
                    .store(cx)
                    .connections()
                    .iter()
                    .find(|c| c.id == id)
                    .cloned()
                {
                    if delete {
                        self.request_connection_delete(conn, window, cx)
                    } else {
                        self.edit(Some(conn), window, cx)
                    }
                }
            }
        }
        cx.notify();
    }

    pub(super) fn choices_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.popup else {
            return div().absolute().into_any_element();
        };
        let content = div()
            .id("choices")
            .occlude()
            .flex()
            .flex_col()
            .p_1()
            .w(px(168.))
            .rounded_md()
            .border_1()
            .border_color(rgb(current().border))
            .bg(rgb(current().panel))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" {
                    this.dismiss_choices(window, cx);
                } else if let Some(menu) = &this.popup {
                    let back = match e.keystroke.key.as_str() {
                        "up" => true,
                        "down" => false,
                        "tab" => e.keystroke.modifiers.shift,
                        _ => return,
                    };
                    let count = menu.items.len();
                    let index = menu
                        .items
                        .iter()
                        .position(|(_, f)| f.is_focused(window))
                        .unwrap_or(0);
                    window.focus(&menu.items[(index + if back { count - 1 } else { 1 }) % count].1);
                }
                window.prevent_default();
                cx.stop_propagation();
            }))
            .children(
                menu.items
                    .iter()
                    .enumerate()
                    .map(|(index, (choice, focus))| {
                        let id = match choice {
                            Choice::Terminal(t) => format!("settings-palette-{}", t.name()),
                            Choice::Delete(id) => format!("delete-all-{id}"),
                            _ => format!("choice-{index}"),
                        };
                        quiet(("choice", index), choice.label())
                            .debug_selector(move || id.clone())
                            .track_focus(focus)
                            .px_2()
                            .py_1()
                            .when(matches!(choice, Choice::Delete(_)), |b| {
                                b.text_color(rgb(current().danger))
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.pick_choice(index, window, cx)
                            }))
                    }),
            );
        div()
            .id("choices-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.dismiss_choices(window, cx);
                    cx.stop_propagation()
                }),
            )
            .child(
                anchored()
                    .position(menu.position)
                    .snap_to_window_with_margin(Edges::all(px(4.)))
                    .child(content),
            )
            .into_any_element()
    }
}
