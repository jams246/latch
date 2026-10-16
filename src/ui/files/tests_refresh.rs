use super::{
    local,
    pane::Side,
    tests_interaction::{draw, fixture},
};
use gpui::{MouseButton, TestAppContext};
use latch::sftp::Directory;

#[gpui::test]
fn context_refresh_reloads_only_its_pane_with_or_without_a_listing(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    for has_listing in [false, true] {
        handle
            .update(&mut visual, |files, _, _| {
                files.local = has_listing.then(|| local::Directory {
                    path: files.initial_directory.clone().unwrap(),
                    entries: Vec::new(),
                    drives: Vec::new(),
                });
                files.directory = has_listing.then(|| Directory {
                    path: "/synthetic".into(),
                    entries: Vec::new(),
                });
            })
            .unwrap();
        draw(handle, &mut visual);
        for (side, selector) in [
            (Side::Local, "local-entries"),
            (Side::Remote, "remote-entries"),
        ] {
            let position = visual.debug_bounds(selector).unwrap().center();
            visual.simulate_mouse_down(position, MouseButton::Right, Default::default());
            visual.simulate_mouse_up(position, MouseButton::Right, Default::default());
            draw(handle, &mut visual);
            handle
                .read_with(&visual, |files, _| {
                    let menu = files.menu.as_ref().unwrap();
                    assert_eq!(menu.side, side);
                    assert_eq!(menu.parent.is_some(), has_listing);
                })
                .unwrap();
            let position = visual.debug_bounds("file-action-4").unwrap().center();
            visual.simulate_click(position, Default::default());
            handle
                .update(&mut visual, |files, _, _| {
                    assert!(files.menu.is_none() && files.dialog.is_none());
                    assert_eq!(files.local_busy, side == Side::Local);
                    assert_eq!(files.busy, side == Side::Remote);
                    assert_eq!(files.local_operation.is_some(), side == Side::Local);
                    assert_eq!(files.operation.is_some(), side == Side::Remote);
                    for operation in [files.local_operation.take(), files.operation.take()]
                        .into_iter()
                        .flatten()
                    {
                        operation.abort();
                    }
                    files.local_busy = false;
                    files.busy = false;
                })
                .unwrap();
            draw(handle, &mut visual);
        }
    }
}
