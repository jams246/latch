use super::Recovery;
use gpui::{IntoElement, TestAppContext, point, px, size};
use std::{path::PathBuf, sync::Arc};

#[gpui::test]
fn locked_profile_should_offer_exit_without_import(cx: &mut TestAppContext) {
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (entity, visual) = cx.add_window_view(|_, cx| Recovery {
        directory: PathBuf::from("."),
        runtime,
        cause: "Synthetic profile lock".into(),
        error: None,
        allow_import: false,
        busy: false,
        focus: cx.focus_handle(),
    });
    visual.draw(point(px(0.), px(0.)), size(px(680.), px(460.)), |_, _| {
        entity.clone().into_any_element()
    });
    assert!(visual.debug_bounds("recovery-import").is_none());
    assert!(visual.debug_bounds("recovery-exit").is_some());
}
