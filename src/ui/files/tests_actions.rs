use super::{
    Files,
    actions::{ActionKind, Target},
    local,
    pane::Side,
    tests_interaction::{draw, fixture},
};
use gpui::{KeyUpEvent, Keystroke, MouseButton, TestAppContext, VisualTestContext, WindowHandle};
use latch::sftp::{Directory, Entry};
use std::{
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

fn populated(cx: &mut TestAppContext) -> (WindowHandle<Files>, VisualTestContext, [PathBuf; 2]) {
    let (handle, mut visual) = fixture(cx);
    let paths = handle
        .update(&mut visual, |files, _, _| {
            let parent = files.initial_directory.clone().unwrap();
            let paths = [parent.join("first.txt"), parent.join("second.txt")];
            let entries: Vec<_> = paths
                .iter()
                .map(|path| {
                    fs::write(path, b"keep").unwrap();
                    Entry {
                        name: path.file_name().unwrap().to_string_lossy().into_owned(),
                        path: path.to_string_lossy().into_owned(),
                        is_dir: false,
                        is_link: false,
                        size: Some(4),
                        modified: None,
                    }
                })
                .collect();
            files.local = Some(local::Directory {
                path: parent,
                entries: paths
                    .iter()
                    .zip(&entries)
                    .map(|(path, metadata)| local::Entry {
                        metadata: metadata.clone(),
                        path: path.clone(),
                        icon: None,
                    })
                    .collect(),
                drives: vec![],
            });
            files.directory = Some(Directory {
                path: "/synthetic".into(),
                entries: entries
                    .into_iter()
                    .map(|mut entry| {
                        entry.path = format!("/synthetic/{}", entry.name);
                        entry
                    })
                    .collect(),
            });
            paths
        })
        .unwrap();
    draw(handle, &mut visual);
    (handle, visual, paths)
}

fn open_action(
    handle: WindowHandle<Files>,
    visual: &mut VisualTestContext,
    side: Side,
    kind: ActionKind,
) {
    let selector = match side {
        Side::Local => "local-entry-0",
        Side::Remote => "remote-entry-0",
    };
    let position = visual.debug_bounds(selector).unwrap().center();
    visual.simulate_mouse_down(position, MouseButton::Right, Default::default());
    visual.simulate_mouse_up(position, MouseButton::Right, Default::default());
    draw(handle, visual);
    let selector = match kind {
        ActionKind::Rename => "file-action-0",
        ActionKind::Delete => "file-action-1",
        _ => panic!("This fixture opens rename or delete"),
    };
    let position = visual.debug_bounds(selector).unwrap().center();
    visual.simulate_click(position, Default::default());
    draw(handle, visual);
}

#[gpui::test]
fn delete_requires_confirmation_and_enter_defaults_to_cancel_in_both_panes(
    cx: &mut TestAppContext,
) {
    let (handle, mut visual, paths) = populated(cx);
    for side in [Side::Local, Side::Remote] {
        open_action(handle, &mut visual, side, ActionKind::Delete);
        handle
            .update(&mut visual, |files, window, _| {
                let dialog = files.dialog.as_ref().unwrap();
                assert_eq!(dialog.kind, ActionKind::Delete);
                assert!(dialog.cancel.is_focused(window));
                assert!(!files.action_busy && files.action_task.is_none());
            })
            .unwrap();
        assert!(paths.iter().all(|path| path.is_file()));
        visual.simulate_keystrokes("enter");
        visual.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
        });
        handle
            .read_with(&visual, |files, _| {
                assert!(files.dialog.is_none());
                assert!(!files.action_busy && files.action_task.is_none());
            })
            .unwrap();
        assert!(paths.iter().all(|path| path.is_file()));
        draw(handle, &mut visual);
    }
}

#[gpui::test]
fn clicking_cancel_leaves_selected_file_intact(cx: &mut TestAppContext) {
    let (handle, mut visual, paths) = populated(cx);
    open_action(handle, &mut visual, Side::Local, ActionKind::Delete);
    let position = visual.debug_bounds("cancel-file-action").unwrap().center();
    visual.simulate_click(position, Default::default());
    handle
        .read_with(&visual, |files, _| {
            assert!(files.dialog.is_none() && files.action_task.is_none());
        })
        .unwrap();
    assert_eq!(fs::read(&paths[0]).unwrap(), b"keep");
}

#[gpui::test]
fn confirming_delete_uses_captured_targets_even_after_selection_changes(cx: &mut TestAppContext) {
    let (handle, mut visual, paths) = populated(cx);
    handle
        .update(&mut visual, |files, _, _| {
            files.runtime = Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .unwrap(),
            );
        })
        .unwrap();
    open_action(handle, &mut visual, Side::Local, ActionKind::Delete);
    handle
        .update(&mut visual, |files, _, _| {
            files.selections[0].context(1);
            assert_eq!(
                files.dialog.as_ref().unwrap().targets,
                vec![Target::Local(paths[0].clone())]
            );
            assert!(files.action_task.is_none());
        })
        .unwrap();
    let position = visual.debug_bounds("confirm-file-action").unwrap().center();
    visual.simulate_click(position, Default::default());
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let completed = handle
            .update(&mut visual, |files, _, cx| {
                files.poll_actions(cx);
                !files.action_busy
            })
            .unwrap();
        if completed {
            break;
        }
        assert!(Instant::now() < deadline, "Delete worker did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!paths[0].exists());
    assert_eq!(fs::read(&paths[1]).unwrap(), b"keep");
    handle
        .read_with(&visual, |files, _| {
            assert!(files.dialog.is_none() && files.action_task.is_none());
            assert!(files.local_busy);
        })
        .unwrap();
}

#[gpui::test]
fn invalid_rename_keeps_dialog_open_without_starting_a_worker(cx: &mut TestAppContext) {
    let (handle, mut visual, paths) = populated(cx);
    open_action(handle, &mut visual, Side::Local, ActionKind::Rename);
    handle
        .update(&mut visual, |files, _, cx| {
            let input = files
                .dialog
                .as_ref()
                .unwrap()
                .input
                .as_ref()
                .unwrap()
                .clone();
            input.update(cx, |input, cx| {
                input.take(cx);
                input.replace(0..0, "../escape", cx);
            });
        })
        .unwrap();
    draw(handle, &mut visual);
    let position = visual.debug_bounds("confirm-file-action").unwrap().center();
    visual.simulate_click(position, Default::default());
    handle
        .read_with(&visual, |files, _| {
            assert!(files.dialog.as_ref().unwrap().error.is_some());
            assert!(!files.action_busy && files.action_task.is_none());
        })
        .unwrap();
    assert_eq!(fs::read(&paths[0]).unwrap(), b"keep");
    assert!(
        !paths[0]
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("escape")
            .exists()
    );
}
