use crate::ui::files::{
    Files, local,
    pane::Side,
    tests_interaction::{draw, fixture},
};
use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext,
    WindowHandle,
};
use latch::sftp::{Directory, Entry};
use std::collections::BTreeSet;

fn row_selector(side: Side, index: usize) -> &'static str {
    match side {
        Side::Local => [
            "local-entry-0",
            "local-entry-1",
            "local-entry-2",
            "local-entry-3",
        ][index],
        Side::Remote => [
            "remote-entry-0",
            "remote-entry-1",
            "remote-entry-2",
            "remote-entry-3",
        ][index],
    }
}

fn parent_selector(side: Side) -> &'static str {
    match side {
        Side::Local => "local-parent",
        Side::Remote => "remote-parent",
    }
}

fn populated(cx: &mut TestAppContext) -> (WindowHandle<Files>, VisualTestContext) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |files, _, _| {
            let entries: Vec<_> = ["zeta.txt", "omega", "middle.log", "link"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| Entry {
                    name: name.into(),
                    path: format!("/root/{name}"),
                    is_dir: index == 1,
                    is_link: index == 3,
                    size: Some(1),
                    modified: Some(0),
                })
                .collect();
            files.local = Some(local::Directory {
                path: files.initial_directory.clone().unwrap(),
                drives: vec![],
                entries: entries
                    .iter()
                    .map(|entry| local::Entry {
                        path: files.initial_directory.as_ref().unwrap().join(&entry.name),
                        metadata: entry.clone(),
                        icon: None,
                    })
                    .collect(),
            });
            files.directory = Some(Directory {
                path: "/root".into(),
                entries,
            });
        })
        .unwrap();
    draw(handle, &mut visual);
    (handle, visual)
}

fn selected(
    handle: WindowHandle<Files>,
    visual: &VisualTestContext,
    side: Side,
) -> BTreeSet<usize> {
    handle
        .read_with(visual, |files, _| {
            files.selections[usize::from(side == Side::Remote)]
                .selected
                .clone()
        })
        .unwrap()
}

#[gpui::test]
fn modifier_clicks_select_display_ranges_and_toggle_files_and_folders(cx: &mut TestAppContext) {
    let (handle, mut visual) = populated(cx);
    for side in [Side::Local, Side::Remote] {
        for (index, modifiers, expected) in [
            (1, Modifiers::default(), BTreeSet::from([1])),
            (
                0,
                Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                BTreeSet::from([0, 1, 2, 3]),
            ),
            (
                1,
                Modifiers {
                    control: true,
                    ..Modifiers::default()
                },
                BTreeSet::from([0, 2, 3]),
            ),
            (2, Modifiers::default(), BTreeSet::from([2])),
        ] {
            let position = visual
                .debug_bounds(row_selector(side, index))
                .unwrap()
                .center();
            visual.simulate_click(position, modifiers);
            assert_eq!(selected(handle, &visual, side), expected);
            draw(handle, &mut visual);
        }
    }
    assert_eq!(selected(handle, &visual, Side::Local), BTreeSet::from([2]));
}

#[gpui::test]
fn sorting_keeps_selected_items_and_parent_row_first(cx: &mut TestAppContext) {
    let (handle, mut visual) = populated(cx);
    for side in [Side::Local, Side::Remote] {
        let position = visual.debug_bounds(row_selector(side, 1)).unwrap().center();
        visual.simulate_click(position, Modifiers::default());
        let position = visual
            .debug_bounds(match side {
                Side::Local => "local-column-0",
                Side::Remote => "remote-column-0",
            })
            .unwrap()
            .center();
        visual.simulate_click(position, Modifiers::default());
        draw(handle, &mut visual);
        assert_eq!(selected(handle, &visual, side), BTreeSet::from([1]));
        let parent = visual.debug_bounds(parent_selector(side)).unwrap();
        for index in 0..4 {
            let entry = visual.debug_bounds(row_selector(side, index)).unwrap();
            assert!(parent.origin.y < entry.origin.y);
        }
        let folder = visual.debug_bounds(row_selector(side, 1)).unwrap();
        let first_file = visual.debug_bounds(row_selector(side, 0)).unwrap();
        assert!(folder.origin.y < first_file.origin.y);
        let position = visual.debug_bounds(row_selector(side, 2)).unwrap().center();
        visual.simulate_click(
            position,
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(selected(handle, &visual, side), BTreeSet::from([0, 1, 2]));
        draw(handle, &mut visual);
    }
}

#[gpui::test]
fn right_click_preserves_existing_multiple_selection(cx: &mut TestAppContext) {
    let (handle, mut visual) = populated(cx);
    for index in [0, 1] {
        let position = visual
            .debug_bounds(row_selector(Side::Remote, index))
            .unwrap()
            .center();
        visual.simulate_click(
            position,
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
        );
        draw(handle, &mut visual);
    }
    let position = visual.debug_bounds("remote-entry-0").unwrap().center();
    visual.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
    visual.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());
    assert_eq!(
        selected(handle, &visual, Side::Remote),
        BTreeSet::from([0, 1])
    );
    handle
        .read_with(&visual, |files, _| assert!(files.menu.is_some()))
        .unwrap();
}

#[gpui::test]
fn parent_row_navigates_only_on_double_click_and_is_never_selected(cx: &mut TestAppContext) {
    let (handle, mut visual) = populated(cx);
    for side in [Side::Local, Side::Remote] {
        let position = visual.debug_bounds(parent_selector(side)).unwrap().center();
        visual.simulate_click(position, Modifiers::default());
        handle
            .read_with(&visual, |files, _| {
                assert!(!files.local_busy && !files.busy);
            })
            .unwrap();
        visual.simulate_event(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 2,
            modifiers: Modifiers::default(),
            first_mouse: false,
        });
        visual.simulate_event(MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 2,
            modifiers: Modifiers::default(),
        });
        assert!(selected(handle, &visual, side).is_empty());
        handle
            .update(&mut visual, |files, _, _| match side {
                Side::Local => {
                    assert!(files.local_busy);
                    files.local_busy = false;
                    if let Some(task) = files.local_operation.take() {
                        task.abort();
                    }
                }
                Side::Remote => {
                    assert!(files.busy);
                    files.busy = false;
                    if let Some(task) = files.operation.take() {
                        task.abort();
                    }
                }
            })
            .unwrap();
        draw(handle, &mut visual);
    }
}
