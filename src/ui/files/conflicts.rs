use super::Files;
use crate::ui::button;
use gpui::{prelude::*, *};
use latch::sftp::{Conflict, ConflictChoice};
use std::cell::Cell;

#[cfg(test)]
mod tests;

const CHOICES: [(ConflictChoice, &str); 4] = [
    (ConflictChoice::Skip, "Skip"),
    (ConflictChoice::Replace, "Replace"),
    (ConflictChoice::Duplicate, "Duplicate"),
    (ConflictChoice::Merge, "Merge"),
];

pub(super) struct Dialog {
    pub(super) request: Conflict,
    focus: FocusHandle,
    options: [FocusHandle; 4],
    cancel: FocusHandle,
    focus_pending: Cell<bool>,
}

impl Dialog {
    pub(super) fn new(request: Conflict, cx: &mut Context<Files>) -> Self {
        Self {
            request,
            focus: cx.focus_handle(),
            options: std::array::from_fn(|_| cx.focus_handle()),
            cancel: cx.focus_handle(),
            focus_pending: Cell::new(true),
        }
    }

    fn enabled(&self, choice: ConflictChoice) -> bool {
        choice != ConflictChoice::Merge || self.request.can_merge
    }

    fn focused_choice(&self, window: &Window) -> Option<ConflictChoice> {
        self.options
            .iter()
            .position(|focus| focus.is_focused(window))
            .map(|index| CHOICES[index].0)
            .filter(|choice| self.enabled(*choice))
    }

    fn focus_next(&self, backwards: bool, window: &mut Window) {
        let count = if self.request.can_merge { 4 } else { 3 };
        let current = self
            .options
            .iter()
            .position(|focus| focus.is_focused(window))
            .unwrap_or(count);
        let next = if backwards {
            (current + count) % (count + 1)
        } else {
            (current + 1) % (count + 1)
        };
        window.focus(if next == count {
            &self.cancel
        } else {
            &self.options[next]
        });
    }
}

impl Files {
    fn cancel_conflict(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .conflict
            .as_ref()
            .is_some_and(|dialog| dialog.request.id == id)
            && let Some(transfer) = &self.transfer
        {
            transfer.cancel();
            self.conflict = None;
            window.focus(&self.path.read(cx).focus);
            cx.notify();
        }
    }

    fn choose_conflict(
        &mut self,
        id: u64,
        choice: ConflictChoice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .conflict
            .as_ref()
            .is_none_or(|dialog| dialog.request.id != id || !dialog.enabled(choice))
            || !self
                .transfer
                .as_ref()
                .is_some_and(|transfer| transfer.resolve_conflict(id, choice))
        {
            return;
        }
        self.conflict = None;
        window.focus(&self.path.read(cx).focus);
        cx.notify();
    }

    pub(in crate::ui) fn render_conflict_dialog(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(dialog) = &self.conflict else {
            return div().absolute().into_any_element();
        };
        if dialog.focus_pending.replace(false) {
            window.focus(&dialog.options[0]);
        }
        let id = dialog.request.id;
        div()
            .id("transfer-conflict-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000a0))
            .child(
                div()
                    .id("transfer-conflict-dialog")
                    .debug_selector(|| "transfer-conflict-dialog".into())
                    .track_focus(&dialog.focus)
                    .tab_group()
                    .occlude()
                    .w(px(520.))
                    .max_w_full()
                    .p_4()
                    .gap_3()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .bg(rgb(0x1c2c3a))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" => this.cancel_conflict(id, window, cx),
                            "tab" => {
                                if let Some(dialog) = &this.conflict
                                    && dialog.request.id == id
                                {
                                    dialog.focus_next(event.keystroke.modifiers.shift, window);
                                }
                            }
                            _ => (),
                        }
                        cx.stop_propagation();
                    }))
                    .on_key_up(cx.listener(move |this, event: &KeyUpEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            if this.conflict.as_ref().is_some_and(|dialog| {
                                dialog.request.id == id && dialog.cancel.is_focused(window)
                            }) {
                                this.cancel_conflict(id, window, cx);
                            } else if let Some(choice) = this
                                .conflict
                                .as_ref()
                                .filter(|dialog| dialog.request.id == id)
                                .and_then(|dialog| dialog.focused_choice(window))
                            {
                                this.choose_conflict(id, choice, window, cx);
                            }
                        }
                        cx.stop_propagation();
                    }))
                    .child(div().text_lg().child("Item exists at the destination"))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Source: {}", dialog.request.source)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Destination: {}", dialog.request.destination)),
                    )
                    .child(div().text_sm().text_color(rgb(0xa8c1d3)).child(
                        "Replace overwrites the destination. Duplicate adds a number to the name.",
                    ))
                    .when(dialog.request.can_merge, |view| {
                        view.child(
                            div().text_sm().text_color(rgb(0xa8c1d3)).child(
                                "Merge combines folders and numbers files that already exist.",
                            ),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .children(CHOICES.into_iter().enumerate().map(
                                |(index, (choice, label))| {
                                    let enabled = dialog.enabled(choice);
                                    button(("transfer-conflict-choice", index), label)
                                        .debug_selector(move || {
                                            format!("transfer-conflict-choice-{index}")
                                        })
                                        .track_focus(&dialog.options[index])
                                        .flex_1()
                                        .text_center()
                                        .when(choice == ConflictChoice::Replace, |button| {
                                            button.text_color(rgb(0xff7777))
                                        })
                                        .when(!enabled, |button| button.opacity(0.35).tab_index(-1))
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            if enabled {
                                                this.choose_conflict(id, choice, window, cx);
                                            }
                                        }))
                                },
                            )),
                    )
                    .child(
                        div().flex().justify_end().child(
                            button("cancel-transfer-conflict", "Cancel transfer")
                                .debug_selector(|| "cancel-transfer-conflict".into())
                                .track_focus(&dialog.cancel)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.cancel_conflict(id, window, cx);
                                })),
                        ),
                    ),
            )
            .into_any_element()
    }
}
