use super::{
    TransferEvent,
    actions::ActionResult,
    local,
    pane::Side,
    tests_interaction::{draw, fixture},
};
use gpui::TestAppContext;
use latch::sftp::Entry;
use std::sync::{Arc, mpsc};

fn directory(names: &[&str]) -> local::Directory {
    local::Directory {
        path: "C:/synthetic".into(),
        drives: Vec::new(),
        entries: names
            .iter()
            .map(|name| local::Entry {
                path: format!("C:/synthetic/{name}").into(),
                icon: None,
                metadata: Entry {
                    name: (*name).into(),
                    path: (*name).into(),
                    is_dir: *name == "folder",
                    is_link: false,
                    size: Some(10),
                    modified: None,
                },
            })
            .collect(),
    }
}

#[gpui::test]
fn sorted_indices_survive_redraws_and_progress_but_change_with_sort_or_listing(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual) = fixture(cx);
    let order = handle
        .update(&mut visual, |files, _, cx| {
            (files.local_sender, files.local_events) = mpsc::channel();
            files
                .local_sender
                .send(Ok(directory(&["z.txt", "folder", "a.txt"])))
                .unwrap();
            files.poll(cx);
            assert_eq!(&*files.ordered[0], &[1, 2, 0]);
            files.ordered[0].clone()
        })
        .unwrap();
    for _ in 0..3 {
        draw(handle, &mut visual);
    }
    handle
        .update(&mut visual, |files, _, cx| {
            let (events, progress) = tokio::sync::mpsc::channel(1);
            events
                .try_send(TransferEvent::Progress {
                    name: "a.txt".into(),
                    completed: 1,
                    total: Some(10),
                })
                .unwrap();
            files.progress = Some(progress);
            files.poll(cx);
            files.progress = None;
            assert!(Arc::ptr_eq(&order, &files.ordered[0]));
        })
        .unwrap();
    let position = visual.debug_bounds("local-column-0").unwrap().center();
    visual.simulate_click(position, Default::default());
    let sorted = handle
        .read_with(&visual, |files, _| {
            assert!(!Arc::ptr_eq(&order, &files.ordered[0]));
            assert_eq!(&*files.ordered[0], &[1, 0, 2]);
            files.ordered[0].clone()
        })
        .unwrap();
    handle
        .update(&mut visual, |files, _, cx| {
            files
                .local_sender
                .send(Ok(directory(&["b.txt", "c.txt"])))
                .unwrap();
            files.poll(cx);
            assert!(!Arc::ptr_eq(&sorted, &files.ordered[0]));
            assert_eq!(&*files.ordered[0], &[1, 0]);
        })
        .unwrap();
}

#[gpui::test]
fn action_error_survives_refresh_and_banner_can_be_dismissed(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |files, _, cx| {
            files
                .action_sender
                .send(ActionResult {
                    side: Side::Local,
                    result: Err("Partial deletion: some items remain".into()),
                })
                .unwrap();
            files.poll(cx);
            (files.local_sender, files.local_events) = mpsc::channel();
            files
                .local_sender
                .send(Ok(directory(&["remaining.txt"])))
                .unwrap();
            files.poll(cx);
            assert!(files.error.as_ref().unwrap().contains("Partial deletion"));
        })
        .unwrap();
    draw(handle, &mut visual);
    assert!(visual.debug_bounds("file-error").is_some());
    let title_top = visual.debug_bounds("local-title").unwrap().top();
    let position = visual.debug_bounds("dismiss-file-error").unwrap().center();
    visual.simulate_click(position, Default::default());
    handle
        .read_with(&visual, |files, _| {
            assert!(
                files.error.is_none(),
                "Dismiss must clear the operation error"
            );
            assert!(files.directory_errors.iter().all(Option::is_none));
        })
        .unwrap();
    draw(handle, &mut visual);
    assert!(visual.debug_bounds("local-title").unwrap().top() < title_top);
}

#[gpui::test]
fn transfer_recovery_path_remains_visible_alongside_refresh_failure(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |files, _, cx| {
            let (events, progress) = tokio::sync::mpsc::channel(1);
            events
                .try_send(TransferEvent::Failed(
                    "Original data remains at C:/synthetic/.latch-recovery.backup".into(),
                ))
                .unwrap();
            files.progress = Some(progress);
            files.poll(cx);
            files
                .sender
                .send(Err(latch::sftp::Error::NotConnected))
                .unwrap();
            files.poll(cx);
            assert!(
                files
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("C:/synthetic/.latch-recovery.backup")
            );
            assert!(files.directory_errors[1].is_some());
        })
        .unwrap();
    draw(handle, &mut visual);
    assert!(visual.debug_bounds("file-error").is_some());
}
