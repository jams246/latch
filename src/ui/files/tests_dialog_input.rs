use super::{
    actions::ActionKind,
    pane::Side,
    tests_actions::{open_action, populated},
    tests_interaction::draw,
};
use gpui::{KeyUpEvent, Keystroke, TestAppContext};

#[gpui::test]
fn name_dialogs_accept_typing_and_edits_in_both_panes(cx: &mut TestAppContext) {
    let (handle, mut visual, _) = populated(cx);
    for side in [Side::Local, Side::Remote] {
        for kind in [ActionKind::NewFolder, ActionKind::Rename] {
            open_action(handle, &mut visual, side, kind);
            let input = handle
                .update(&mut visual, |files, window, cx| {
                    let dialog = files.dialog.as_ref().unwrap();
                    let input = dialog.input.as_ref().unwrap().clone();
                    assert_eq!(dialog.side, side);
                    assert_eq!(dialog.kind, kind);
                    assert!(input.read(cx).focus.is_focused(window));
                    input
                })
                .unwrap();
            visual.simulate_keystrokes("ctrl-a f o l d e r");
            input.read_with(&visual, |input, _| {
                assert_eq!(input.value.as_str(), "folder")
            });
            visual.simulate_keystrokes("backspace s");
            input.read_with(&visual, |input, _| {
                assert_eq!(input.value.as_str(), "foldes")
            });
            visual.simulate_keystrokes("ctrl-a backspace enter");
            visual.simulate_event(KeyUpEvent {
                keystroke: Keystroke::parse("enter").unwrap(),
            });
            handle
                .update(&mut visual, |files, window, cx| {
                    assert!(files.dialog.as_ref().unwrap().error.is_some());
                    assert!(!files.action_busy);
                    assert!(input.read(cx).focus.is_focused(window));
                })
                .unwrap();
            visual.simulate_keystrokes("n e w space f o l d e r");
            input.read_with(&visual, |input, _| {
                assert_eq!(input.value.as_str(), "new folder")
            });
            visual.simulate_keystrokes("escape");
            handle
                .update(&mut visual, |files, window, cx| {
                    assert!(files.dialog.is_none());
                    assert!(!files.action_busy);
                    assert!(files.path.read(cx).focus.is_focused(window));
                })
                .unwrap();
            draw(handle, &mut visual);
        }
    }
}
