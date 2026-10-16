use super::{Files, actions::ActionKind};
use crate::ui::button;
use gpui::{prelude::*, *};

impl Files {
    pub(in crate::ui) fn render_context_menu(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(menu) = &self.menu else {
            return div().absolute().into_any_element();
        };
        let selected = menu.targets.len();
        div()
            .id("file-menu-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.close_action_ui(window, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, window, cx| this.close_action_ui(window, cx)),
            )
            .child(
                anchored().position(menu.position).snap_to_window().child(
                    div()
                        .id("file-context-menu")
                        .debug_selector(|| "file-context-menu".into())
                        .track_focus(&menu.focus)
                        .tab_group()
                        .occlude()
                        .w(px(220.))
                        .p_1()
                        .flex()
                        .flex_col()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0x426079))
                        .bg(rgb(0x1c2c3a))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key == "escape" {
                                this.close_action_ui(window, cx);
                            }
                            cx.stop_propagation();
                        }))
                        .children(
                            [
                                (ActionKind::Rename, "Rename", selected == 1),
                                (ActionKind::Delete, "Delete", selected > 0),
                                (ActionKind::NewFolder, "New Folder", true),
                                (ActionKind::Copy, "Copy to target directory", selected > 0),
                            ]
                            .into_iter()
                            .enumerate()
                            .map(|(index, (kind, label, enabled))| {
                                button(("file-action", index), label)
                                    .debug_selector(move || format!("file-action-{index}"))
                                    .rounded_none()
                                    .bg(rgb(0x1c2c3a))
                                    .py_2()
                                    .when(kind == ActionKind::Delete, |row| {
                                        row.text_color(rgb(0xff7777))
                                    })
                                    .when(!enabled, |row| row.opacity(0.35).tab_index(-1))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        if enabled {
                                            this.choose_action(kind, window, cx);
                                        }
                                    }))
                            }),
                        ),
                ),
            )
            .into_any_element()
    }
}
