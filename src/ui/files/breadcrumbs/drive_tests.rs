use crate::ui::files::{
    local,
    tests_interaction::{draw, fixture},
};
use gpui::TestAppContext;
use std::path::PathBuf;

fn check_drives(cx: &mut TestAppContext, drives: &[&str]) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |files, _, _| {
            files.local = Some(local::Directory {
                path: PathBuf::from(r"C:\Users\test"),
                entries: Vec::new(),
                drives: drives.iter().map(PathBuf::from).collect(),
            });
        })
        .unwrap();
    draw(handle, &mut visual);
    let visible = drives.len() > 1;
    assert_eq!(visual.debug_bounds("local-drives-label").is_some(), visible);
    assert_eq!(visual.debug_bounds("local-drives").is_some(), visible);
    for selector in ["local-drive-0", "local-drive-1"] {
        assert_eq!(visual.debug_bounds(selector).is_some(), visible);
    }
}

#[gpui::test]
fn drives_row_is_hidden_without_drives(cx: &mut TestAppContext) {
    check_drives(cx, &[]);
}

#[gpui::test]
fn drives_row_is_hidden_with_one_drive(cx: &mut TestAppContext) {
    check_drives(cx, &[r"C:\"]);
}

#[gpui::test]
fn drives_row_shows_buttons_with_multiple_drives(cx: &mut TestAppContext) {
    check_drives(cx, &[r"C:\", r"D:\"]);
}
