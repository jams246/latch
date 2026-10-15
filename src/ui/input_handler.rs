use super::input::{Input, single_line};
use gpui::*;
use std::ops::Range;
use zeroize::Zeroizing;

impl Input {
    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let key = &event.keystroke;
        let cursor = self.cursor();
        let previous = self.value[..cursor]
            .char_indices()
            .last()
            .map_or(0, |(index, _)| index);
        let next = self.value[cursor..]
            .chars()
            .next()
            .map_or(cursor, |c| cursor + c.len_utf8());
        match key.key.as_str() {
            "backspace" | "delete" => {
                let range = if !self.selection.is_empty() {
                    self.selection.clone()
                } else if key.key == "backspace" {
                    previous..cursor
                } else {
                    cursor..next
                };
                self.replace(range, "", cx);
            }
            "left" | "right" | "home" | "end" => {
                let offset = match key.key.as_str() {
                    "home" => 0,
                    "end" => self.value.len(),
                    "left" if !key.modifiers.shift && !self.selection.is_empty() => {
                        self.selection.start
                    }
                    "right" if !key.modifiers.shift && !self.selection.is_empty() => {
                        self.selection.end
                    }
                    "left" => previous,
                    _ => next,
                };
                self.select_to(offset, key.modifiers.shift, cx);
            }
            "a" if key.modifiers.control => {
                self.selection = 0..self.value.len();
                self.reversed = false;
                self.marked = None;
                cx.notify();
            }
            "v" if key.modifiers.control => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    let text = Zeroizing::new(text);
                    self.replace(self.selection.clone(), &text, cx);
                }
            }
            "c" | "x" if key.modifiers.control => {
                if !self.masked && !self.selection.is_empty() {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        self.value[self.selection.clone()].to_string(),
                    ));
                    if key.key == "x" {
                        self.replace(self.selection.clone(), "", cx);
                    }
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl EntityInputHandler for Input {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        if self.masked || range.start > range.end {
            return None;
        }
        let bytes = self.byte_offset(range.start)..self.byte_offset(range.end);
        *actual = Some(self.utf16_range(bytes.clone()));
        Some(self.value[bytes].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16_range(self.selection.clone()),
            reversed: self.reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone().map(|range| self.utf16_range(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .map(|r| self.byte_offset(r.start)..self.byte_offset(r.end))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        self.replace(range, text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .map(|r| self.byte_offset(r.start)..self.byte_offset(r.end))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.clone());
        let start = range.start;
        let text = single_line(text);
        if self.replace(range, &text, cx) && !text.is_empty() {
            self.marked = Some(start..start + text.len());
            if let Some(range) = selected {
                let from = super::input::utf16_to_byte(&text, range.start);
                let to = super::input::utf16_to_byte(&text, range.end).max(from);
                self.selection = start + from..start + to;
            }
        }
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (Some(viewport), Some(line)) = (self.bounds, self.layout.as_ref()) else {
            return Some(bounds);
        };
        let start =
            line.x_for_index(self.display_offset(self.byte_offset(range.start))) - self.scroll;
        let end = line.x_for_index(self.display_offset(self.byte_offset(range.end))) - self.scroll;
        Some(Bounds::new(
            point(
                viewport.left() + start.clamp(px(0.), viewport.size.width),
                viewport.top(),
            ),
            size(
                (end - start).max(px(1.)).min(viewport.size.width),
                viewport.size.height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(
            self.value[..self.offset_for_point(position)]
                .encode_utf16()
                .count(),
        )
    }
}
