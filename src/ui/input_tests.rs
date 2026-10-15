use super::input::{
    clamp_byte, display_offset, replace_value, selection_to, source_offset, utf16_to_byte,
};
use zeroize::Zeroizing;

#[test]
fn unicode_offsets_map_surrogates_to_valid_utf8_boundaries() {
    let value = "a😀éz";
    assert_eq!(
        (0..=6)
            .map(|offset| utf16_to_byte(value, offset))
            .collect::<Vec<_>>(),
        [0, 1, 5, 5, 7, 8, 8]
    );
    assert_eq!((clamp_byte(value, 3), clamp_byte(value, 100)), (1, 8));
}

#[test]
fn masked_positions_map_bullets_without_exposing_source_text() {
    let value = "a😀éz";
    for byte in [0, 1, 5, 7, 8] {
        assert_eq!(
            source_offset(value, display_offset(value, byte, true), true),
            byte
        );
    }
    assert_eq!(source_offset(value, usize::MAX, true), value.len());
}

#[test]
fn replacement_rejects_invalid_ranges_and_disabled_edits() {
    let mut value = Zeroizing::new("a😀éz".to_owned());
    for (range, disabled) in [
        (2..5, false),
        (std::ops::Range { start: 5, end: 2 }, false),
        (0..100, false),
        (0..1, true),
    ] {
        assert_eq!(replace_value(&mut value, range, "x", disabled), None);
        assert_eq!(&**value, "a😀éz");
    }
}

#[test]
fn replacement_handles_unicode_selection_and_single_line_paste() {
    let mut value = Zeroizing::new("a😀éz".to_owned());
    assert_eq!(replace_value(&mut value, 1..7, "雪\r\n", false), Some(4));
    assert_eq!(&**value, "a雪z");
    assert_eq!(replace_value(&mut value, 1..4, "", false), Some(1));
    assert_eq!(&**value, "az");
}

#[test]
fn shift_selection_preserves_anchor_when_crossing_it() {
    let (selection, reversed) = selection_to(3..3, false, 1, true);
    assert_eq!((selection.clone(), reversed), (1..3, true));
    let (selection, reversed) = selection_to(selection, reversed, 5, true);
    assert_eq!((selection.clone(), reversed), (3..5, false));
    assert_eq!(selection_to(selection, reversed, 2, false), (2..2, false));
}
