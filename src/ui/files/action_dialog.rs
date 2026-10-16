use super::{Files, actions::ActionKind};
use crate::ui::{button, theme::CARBON};
use gpui::{prelude::*, *};

impl Files {
    pub(in crate::ui) fn render_action_dialog(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(dialog) = &self.dialog else {
            return div().absolute().into_any_element();
        };
        let (title, confirm) = match dialog.kind {
            ActionKind::Rename => ("Rename", "Rename"),
            ActionKind::NewFolder => ("New Folder", "Create folder"),
            ActionKind::Delete => ("Delete selected items?", "Delete"),
            ActionKind::Copy | ActionKind::Refresh => return div().absolute().into_any_element(),
        };
        let subject = if dialog.targets.len() == 1 {
            format!("\"{}\"", super::actions::target_name(&dialog.targets[0]))
        } else {
            format!("{} selected items", dialog.targets.len())
        };
        div().id("file-action-dialog-overlay")
            .absolute().inset_0().occlude().flex().items_center().justify_center()
            .bg(rgba(CARBON.overlay))
            .child(div().id("file-action-dialog")
                .debug_selector(|| "file-action-dialog".into())
                .track_focus(&dialog.focus).tab_group().occlude()
                .w(px(370.)).max_w_full().flex().flex_col()
                .rounded_lg().overflow_hidden().bg(rgb(CARBON.panel))
                .text_color(rgb(CARBON.text))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    match event.keystroke.key.as_str() {
                        "escape" => this.close_action_ui(window, cx),
                        "tab" => {
                            if event.keystroke.modifiers.shift { window.focus_prev(); } else { window.focus_next(); }
                            if let Some(dialog) = &this.dialog && !dialog.focus.contains_focused(window, cx) {
                                let focus = if event.keystroke.modifiers.shift {
                                    dialog.confirm.clone()
                                } else {
                                    dialog.input.as_ref().map_or_else(|| dialog.cancel.clone(), |input| input.read(cx).focus.clone())
                                };
                                window.focus(&focus);
                            }
                        },
                        _ => (),
                    }
                    cx.stop_propagation();
                }))
                .on_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                    if event.keystroke.key == "enter" && this.dialog.as_ref().is_some_and(|d| d.cancel.is_focused(window)) {
                        this.close_action_ui(window, cx);
                    } else if event.keystroke.key == "enter" && this.dialog.as_ref().is_some_and(|d| d.confirm.is_focused(window) || d.input.as_ref().is_some_and(|i| i.read(cx).focus.is_focused(window))) {
                        this.submit_action(window, cx);
                    }
                    cx.stop_propagation();
                }))
                .child(crate::ui::dialog_header(title))
                .child(div().flex().flex_col().gap_3().p_4()
                .when(dialog.kind == ActionKind::Delete, |view| view
                    .child(format!("{}: delete {subject}, including folder contents?", if dialog.side == super::pane::Side::Local { "Local computer" } else { "Connected server" }))
                    .child(div().text_color(rgb(CARBON.danger)).child("This cannot be undone. Links are removed without deleting their targets.")))
                .children(dialog.input.iter().map(|input| div().child(input.clone())))
                .children(dialog.error.iter().map(|error| div().text_color(rgb(CARBON.danger)).child(error.clone())))
                .child(div().flex().gap_2().justify_end()
                    .child(button("cancel-file-action", "Cancel")
                        .track_focus(&dialog.cancel)
                        .debug_selector(|| "cancel-file-action".into())
                        .on_click(cx.listener(|this, _, window, cx| this.close_action_ui(window, cx))))
                    .child(button("confirm-file-action", confirm)
                        .track_focus(&dialog.confirm)
                        .debug_selector(|| "confirm-file-action".into())
                        .when(dialog.kind == ActionKind::Delete, |button| button.border_color(rgb(CARBON.danger)))
                        .on_click(cx.listener(|this, _, window, cx| this.submit_action(window, cx)))))))
            .into_any_element()
    }
}
