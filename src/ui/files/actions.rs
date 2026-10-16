use super::*;
mod state;
pub(super) use state::*;

impl Files {
    pub(super) fn actions_idle(&self) -> bool {
        !self.closing
            && !self.reconnecting
            && !self.busy
            && !self.local_busy
            && !self.action_busy
            && self.transfer.is_none()
    }

    pub(in crate::ui) fn open_context_menu(
        &mut self,
        side: Side,
        row: Option<usize>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.actions_idle() || self.dialog.is_some() {
            return;
        }
        let index = usize::from(side == Side::Remote);
        if row.is_none() {
            self.selections[index].clear();
        }
        let parent = self.pane_directory(side);
        let targets = self.selected_targets(side);
        let focus = cx.focus_handle();
        window.focus(&focus);
        self.menu = Some(ContextMenu {
            side,
            position,
            focus,
            targets,
            parent,
        });
        cx.notify();
    }

    fn pane_directory(&self, side: Side) -> Option<Target> {
        match side {
            Side::Local => self.local.as_ref().map(|d| Target::Local(d.path.clone())),
            Side::Remote => self
                .directory
                .as_ref()
                .map(|d| Target::Remote(d.path.clone())),
        }
    }

    fn selected_targets(&self, side: Side) -> Vec<Target> {
        self.selections[usize::from(side == Side::Remote)]
            .selected
            .iter()
            .filter_map(|index| match side {
                Side::Local => self
                    .local
                    .as_ref()?
                    .entries
                    .get(*index)
                    .map(|e| Target::Local(e.path.clone())),
                Side::Remote => self
                    .directory
                    .as_ref()?
                    .entries
                    .get(*index)
                    .map(|e| Target::Remote(e.path.clone())),
            })
            .collect()
    }

    pub(super) fn choose_action(
        &mut self,
        kind: ActionKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.menu.take() else { return };
        if !self.actions_idle() {
            return;
        }
        if kind == ActionKind::Refresh {
            window.focus(&self.path.read(cx).focus);
            self.refresh(menu.side, cx);
            return;
        }
        if kind == ActionKind::Copy {
            self.copy_targets(menu.targets, cx);
            window.focus(&self.path.read(cx).focus);
            return;
        }
        if (kind == ActionKind::Rename && menu.targets.len() != 1)
            || (kind == ActionKind::Delete && menu.targets.is_empty())
        {
            return;
        }
        let Some(parent) = menu.parent else { return };
        let input = (kind != ActionKind::Delete).then(|| {
            let name = if kind == ActionKind::Rename {
                menu.targets.first().map(target_name).unwrap_or_default()
            } else {
                String::new()
            };
            cx.new(|cx| Input::new(&name, false, cx))
        });
        let focus = cx.focus_handle().tab_stop(false);
        let cancel = cx.focus_handle().tab_index(0).tab_stop(true);
        let confirm = cx.focus_handle().tab_index(0).tab_stop(true);
        window.focus(
            &input
                .as_ref()
                .map_or_else(|| cancel.clone(), |input| input.read(cx).focus.clone()),
        );
        self.dialog = Some(ActionDialog {
            kind,
            side: menu.side,
            targets: menu.targets,
            parent,
            input,
            focus,
            cancel,
            confirm,
            error: None,
        });
        cx.notify();
    }

    pub(super) fn close_action_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.dialog = None;
        window.focus(&self.path.read(cx).focus);
        cx.notify();
    }

    pub(super) fn submit_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.actions_idle() {
            return;
        }
        let Some(dialog) = &self.dialog else { return };
        let name = dialog
            .input
            .as_ref()
            .map(|input| input.read(cx).value.to_string());
        if let Some(name) = &name {
            let validation = match dialog.side {
                Side::Local => local::validate_name(name).map_err(|e| e.to_string()),
                Side::Remote => validate_remote_name(name).map_err(str::to_string),
            };
            if let Err(error) = validation {
                if let Some(dialog) = &mut self.dialog {
                    dialog.error = Some(error);
                }
                cx.notify();
                return;
            }
        }
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        self.start_action(dialog, name.unwrap_or_default(), cx);
        window.focus(&self.path.read(cx).focus);
    }
}

pub(super) fn target_name(target: &Target) -> String {
    match target {
        Target::Local(path) => path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        Target::Remote(path) => path.rsplit('/').next().unwrap_or_default().to_string(),
    }
}

fn validate_remote_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() || matches!(name, "." | "..") || name.contains(['/', '\\', '\0']) {
        Err("Enter one filename without a path or parent-directory component.")
    } else {
        Ok(())
    }
}
