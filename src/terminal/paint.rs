use alacritty_terminal::{term::cell::Flags, vte::ansi::CursorShape};
use gpui::{
    App, Bounds, FontWeight, Pixels, TextRun, Window, fill, font, point, px, rgb, rgba, size,
};

use super::{colors, state::Snapshot};

pub(super) const LINE_HEIGHT: Pixels = px(20.);
const FONT_SIZE: Pixels = px(14.);

pub(super) fn cell_width(window: &Window) -> Pixels {
    let run = TextRun {
        len: 1,
        font: font("Cascadia Mono"),
        color: rgb(0xffffff).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line("M".into(), FONT_SIZE, &[run], None)
        .width
}

pub(super) fn paint(
    bounds: Bounds<Pixels>,
    snapshot: Snapshot,
    width: Pixels,
    composition: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let mut row_text = String::new();
    let mut runs = Vec::new();
    let mut row = 0;
    let cursor = snapshot.cursor;
    for visible in &snapshot.cells {
        if visible.row != row {
            paint_row(&mut row_text, &mut runs, row, bounds, window, cx);
            row = visible.row;
        }
        let cell = &visible.cell;
        let flags = cell.flags;
        let mut fg = colors::resolve(cell.fg, &snapshot.colors);
        let mut bg = colors::resolve(cell.bg, &snapshot.colors);
        if flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if flags.contains(Flags::DIM) {
            fg = (fg & 0xfefefe) >> 1;
        }
        if visible.selected {
            bg = 0x365578;
        }
        let position = point(
            bounds.left() + width * visible.column as f32,
            bounds.top() + LINE_HEIGHT * row as f32,
        );
        let cell_bounds = Bounds::new(position, size(width, LINE_HEIGHT));
        window.paint_quad(fill(cell_bounds, rgb(bg)));
        if cursor.0 == row && cursor.1 == visible.column && cursor.2 != CursorShape::Hidden {
            let cursor_bounds = match cursor.2 {
                CursorShape::Beam => Bounds::new(position, size(px(2.), LINE_HEIGHT)),
                CursorShape::Underline => Bounds::new(
                    point(position.x, position.y + LINE_HEIGHT - px(2.)),
                    size(width, px(2.)),
                ),
                _ => cell_bounds,
            };
            window.paint_quad(fill(cursor_bounds, rgba(0xd7dce570)));
        }
        if flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
            continue;
        }
        let start = row_text.len();
        row_text.push(if flags.contains(Flags::HIDDEN) {
            ' '
        } else {
            cell.c
        });
        if !flags.contains(Flags::HIDDEN)
            && let Some(combining) = cell.zerowidth()
        {
            row_text.extend(combining);
        }
        let mut cell_font = font("Cascadia Mono");
        if flags.contains(Flags::BOLD) {
            cell_font.weight = FontWeight::BOLD;
        }
        if flags.contains(Flags::ITALIC) {
            cell_font = cell_font.italic();
        }
        runs.push(TextRun {
            len: row_text.len() - start,
            font: cell_font,
            color: rgb(fg).into(),
            background_color: None,
            underline: flags
                .intersects(Flags::ALL_UNDERLINES)
                .then_some(gpui::UnderlineStyle {
                    thickness: px(1.),
                    color: None,
                    wavy: flags.contains(Flags::UNDERCURL),
                }),
            strikethrough: flags
                .contains(Flags::STRIKEOUT)
                .then_some(gpui::StrikethroughStyle {
                    thickness: px(1.),
                    color: None,
                }),
        });
    }
    paint_row(&mut row_text, &mut runs, row, bounds, window, cx);
    if !composition.is_empty() && cursor.0 < snapshot.rows {
        let run = TextRun {
            len: composition.len(),
            font: font("Cascadia Mono"),
            color: rgb(0xffffff).into(),
            background_color: Some(rgb(0x365578).into()),
            underline: None,
            strikethrough: None,
        };
        let line =
            window
                .text_system()
                .shape_line(composition.to_owned().into(), FONT_SIZE, &[run], None);
        let position = point(
            bounds.left() + width * cursor.1 as f32,
            bounds.top() + LINE_HEIGHT * cursor.0 as f32,
        );
        let _ = line.paint(position, LINE_HEIGHT, window, cx);
    }
}

fn paint_row(
    text: &mut String,
    runs: &mut Vec<TextRun>,
    row: usize,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    if text.is_empty() {
        return;
    }
    let line = window
        .text_system()
        .shape_line(std::mem::take(text).into(), FONT_SIZE, runs, None);
    let _ = line.paint(
        point(bounds.left(), bounds.top() + LINE_HEIGHT * row as f32),
        LINE_HEIGHT,
        window,
        cx,
    );
    runs.clear();
}
