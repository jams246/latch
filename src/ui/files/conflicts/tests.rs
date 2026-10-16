use super::{CHOICES, Dialog};
use crate::ui::files::tests_interaction::{draw, fixture};
use gpui::TestAppContext;
use latch::sftp::{Conflict, ConflictChoice};

fn request(can_merge: bool) -> Conflict {
    Conflict {
        id: 7,
        source: "source/report.txt".into(),
        destination: "destination/report.txt".into(),
        can_merge,
    }
}

#[gpui::test]
fn dialog_defaults_to_skip_and_keeps_tabs_inside_enabled_choices(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    for can_merge in [false, true] {
        handle
            .update(&mut visual, |files, _, cx| {
                files.conflict = Some(Dialog::new(request(can_merge), cx));
            })
            .unwrap();
        draw(handle, &mut visual);
        let choices = if can_merge { 4 } else { 3 };
        handle
            .update(&mut visual, |files, window, _| {
                let dialog = files.conflict.as_ref().unwrap();
                for (index, choice) in CHOICES
                    .iter()
                    .take(choices)
                    .map(|(choice, _)| Some(*choice))
                    .chain(std::iter::once(None))
                    .enumerate()
                {
                    assert_eq!(dialog.focused_choice(window), choice);
                    assert_eq!(dialog.cancel.is_focused(window), index == choices);
                    dialog.focus_next(false, window);
                }
                dialog.focus_next(true, window);
                dialog.focus_next(true, window);
                assert_eq!(dialog.focused_choice(window), Some(CHOICES[choices - 1].0));
            })
            .unwrap();
    }
}

#[gpui::test]
fn redraw_preserves_choice_focus_and_merge_requires_two_folders(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |files, _, cx| {
            files.conflict = Some(Dialog::new(request(false), cx));
        })
        .unwrap();
    draw(handle, &mut visual);
    handle
        .update(&mut visual, |files, window, _| {
            let dialog = files.conflict.as_ref().unwrap();
            window.focus(&dialog.options[2]);
            assert!(!dialog.enabled(ConflictChoice::Merge));
        })
        .unwrap();
    draw(handle, &mut visual);
    handle
        .update(&mut visual, |files, window, _| {
            let dialog = files.conflict.as_ref().unwrap();
            assert_eq!(
                dialog.focused_choice(window),
                Some(ConflictChoice::Duplicate)
            );
            window.focus(&dialog.options[3]);
            assert_eq!(dialog.focused_choice(window), None);
        })
        .unwrap();
}

#[gpui::test]
fn modal_blocks_underlying_pane_headers(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    let before = handle
        .update(&mut visual, |files, _, cx| {
            files.conflict = Some(Dialog::new(request(false), cx));
            (files.local_sort.descending, files.remote_sort.descending)
        })
        .unwrap();
    draw(handle, &mut visual);
    for selector in ["local-column-0", "remote-column-0"] {
        let position = visual.debug_bounds(selector).unwrap().center();
        visual.simulate_click(position, Default::default());
    }
    handle
        .update(&mut visual, |files, _, _| {
            assert_eq!(
                (files.local_sort.descending, files.remote_sort.descending),
                before
            );
        })
        .unwrap();
}
