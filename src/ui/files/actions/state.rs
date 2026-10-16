use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::ui::files) enum Target {
    Local(PathBuf),
    Remote(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui::files) enum ActionKind {
    Rename,
    Delete,
    NewFolder,
    Copy,
    Refresh,
}

pub(in crate::ui::files) struct ContextMenu {
    pub side: Side,
    pub position: Point<Pixels>,
    pub focus: FocusHandle,
    pub targets: Vec<Target>,
    pub parent: Option<Target>,
}

pub(in crate::ui::files) struct ActionDialog {
    pub kind: ActionKind,
    pub side: Side,
    pub targets: Vec<Target>,
    pub parent: Target,
    pub input: Option<Entity<Input>>,
    pub focus: FocusHandle,
    pub cancel: FocusHandle,
    pub confirm: FocusHandle,
    pub error: Option<String>,
}

pub(in crate::ui::files) struct ActionResult {
    pub side: Side,
    pub result: Result<(), String>,
    pub connection_lost: bool,
}
