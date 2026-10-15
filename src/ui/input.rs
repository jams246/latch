use gpui::*;
use std::ops::Range;
use zeroize::Zeroizing;

pub struct Input {
    pub focus: FocusHandle,
    pub value: Zeroizing<String>,
    pub masked: bool,
    pub disabled: bool,
    pub selection: Range<usize>,
    pub marked: Option<Range<usize>>,
    pub(super) reversed: bool,
    pub(super) dragging: bool,
    pub(super) layout: Option<ShapedLine>,
    pub(super) bounds: Option<Bounds<Pixels>>,
    pub(super) scroll: Pixels,
}

impl Input {
    pub fn new(value: &str, masked: bool, cx: &mut Context<Self>) -> Self {
        let value = single_line(value);
        let end = value.len();
        Self {
            focus: cx.focus_handle(),
            value,
            masked,
            disabled: false,
            selection: end..end,
            marked: None,
            reversed: false,
            dragging: false,
            layout: None,
            bounds: None,
            scroll: px(0.),
        }
    }

    pub fn take(&mut self, cx: &mut Context<Self>) -> Zeroizing<String> {
        self.selection = 0..0;
        self.marked = None;
        self.reversed = false;
        self.layout = None;
        self.scroll = px(0.);
        cx.notify();
        std::mem::take(&mut self.value)
    }

    pub fn replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) -> bool {
        let Some(cursor) = replace_value(&mut self.value, range, text, self.disabled) else {
            return false;
        };
        self.selection = cursor..cursor;
        self.reversed = false;
        self.marked = None;
        cx.notify();
        true
    }

    pub fn byte_offset(&self, utf16: usize) -> usize {
        utf16_to_byte(&self.value, utf16)
    }

    pub fn utf16_range(&self, range: Range<usize>) -> Range<usize> {
        let start = clamp_byte(&self.value, range.start);
        let end = clamp_byte(&self.value, range.end).max(start);
        self.value[..start].encode_utf16().count()..self.value[..end].encode_utf16().count()
    }

    pub(super) fn cursor(&self) -> usize {
        if self.reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    pub(super) fn select_to(&mut self, offset: usize, extend: bool, cx: &mut Context<Self>) {
        let offset = clamp_byte(&self.value, offset);
        (self.selection, self.reversed) =
            selection_to(self.selection.clone(), self.reversed, offset, extend);
        self.marked = None;
        cx.notify();
    }

    pub(super) fn offset_for_point(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(line)) = (self.bounds, self.layout.as_ref()) else {
            return self.value.len();
        };
        source_offset(
            &self.value,
            line.closest_index_for_x(position.x - bounds.left() + self.scroll),
            self.masked,
        )
    }

    pub(super) fn display_offset(&self, byte: usize) -> usize {
        display_offset(&self.value, byte, self.masked)
    }
}

impl Focusable for Input {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub(super) fn single_line(value: &str) -> Zeroizing<String> {
    Zeroizing::new(value.replace(['\r', '\n'], ""))
}

pub(super) fn replace_value(
    value: &mut Zeroizing<String>,
    range: Range<usize>,
    text: &str,
    disabled: bool,
) -> Option<usize> {
    if disabled
        || range.start > range.end
        || range.end > value.len()
        || !value.is_char_boundary(range.start)
        || !value.is_char_boundary(range.end)
    {
        return None;
    }
    let text = single_line(text);
    let mut next = Zeroizing::new(String::with_capacity(
        value.len() - range.len() + text.len(),
    ));
    next.push_str(&value[..range.start]);
    next.push_str(&text);
    next.push_str(&value[range.end..]);
    // Replacing the allocation lets Zeroizing erase the previous text, including removed bytes.
    *value = next;
    Some(range.start + text.len())
}

pub(super) fn utf16_to_byte(value: &str, offset: usize) -> usize {
    let mut units = 0;
    for (index, character) in value.char_indices() {
        if units >= offset {
            return index;
        }
        units += character.len_utf16();
    }
    value.len()
}

pub(super) fn clamp_byte(value: &str, offset: usize) -> usize {
    let mut offset = offset.min(value.len());
    while !value.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

pub(super) fn display_offset(value: &str, byte: usize, masked: bool) -> usize {
    let byte = clamp_byte(value, byte);
    if masked {
        value[..byte].chars().count() * '•'.len_utf8()
    } else {
        byte
    }
}

pub(super) fn source_offset(value: &str, displayed: usize, masked: bool) -> usize {
    if masked {
        value
            .char_indices()
            .nth(displayed / '•'.len_utf8())
            .map_or(value.len(), |(byte, _)| byte)
    } else {
        clamp_byte(value, displayed)
    }
}

pub(super) fn selection_to(
    selection: Range<usize>,
    reversed: bool,
    offset: usize,
    extend: bool,
) -> (Range<usize>, bool) {
    if !extend {
        return (offset..offset, false);
    }
    let anchor = if reversed {
        selection.end
    } else {
        selection.start
    };
    (anchor.min(offset)..anchor.max(offset), offset < anchor)
}
