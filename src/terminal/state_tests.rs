use super::*;
use alacritty_terminal::vte::ansi::{Color, NamedColor};

#[test]
fn ansi_should_move_cursor_and_apply_color() {
    let terminal = TerminalState::new(10, 3);
    terminal.feed(b"\x1b[2;3H\x1b[31mX");
    let snapshot = terminal.snapshot();
    let cell = snapshot
        .cells
        .iter()
        .find(|cell| cell.row == 1 && cell.column == 2)
        .unwrap();
    assert_eq!(
        (
            cell.cell.c,
            cell.cell.fg,
            snapshot.cursor.0,
            snapshot.cursor.1
        ),
        ('X', Color::Named(NamedColor::Red), 1, 3)
    );
}

#[test]
fn scrollback_should_retain_output() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"first\r\nsecond\r\nthird");
    terminal.scroll(1);
    assert_eq!(terminal.snapshot().cells[0].cell.c, 'f');
}

#[test]
fn alternate_screen_should_preserve_shell_content() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"shell\x1b[?1049happ\x1b[?1049l");
    assert_eq!(terminal.snapshot().cells[0].cell.c, 's');
}

#[test]
fn resize_should_keep_valid_minimum_dimensions() {
    let terminal = TerminalState::new(10, 2);
    terminal.resize(0, 0);
    assert_eq!(terminal.snapshot().cells.len(), 2);
}

#[test]
fn selection_should_copy_remote_text() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"hello");
    terminal.select(0, 0, true);
    terminal.select(0, 4, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("hello"));
}

#[test]
fn reverse_selection_should_include_both_boundary_characters() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"hello");
    terminal.select(0, 4, true);
    terminal.select(0, 0, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("hello"));
}

#[test]
fn reverse_multiline_selection_should_include_both_boundary_cells() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"hello\r\nworld");
    terminal.select(1, 4, true);
    terminal.select(0, 0, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("hello\nworld"));
}

#[test]
fn selection_crossing_its_anchor_should_adjust_boundary_sides() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"hello");
    terminal.select(0, 2, true);
    terminal.select(0, 0, false);
    terminal.select(0, 4, false);
    assert_eq!(terminal.selected_text().as_deref(), Some("llo"));
}

#[test]
fn device_status_should_reply_with_cursor_position() {
    let terminal = TerminalState::new(10, 2);
    assert_eq!(terminal.feed(b"abc\x1b[6n"), b"\x1b[1;4R");
}

#[test]
fn split_unicode_should_preserve_parser_state() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(&[0xe2, 0x82]);
    terminal.feed(&[0xac]);
    assert_eq!(terminal.snapshot().cells[0].cell.c, '€');
}

#[test]
fn independent_terminal_should_not_share_output() {
    let first = TerminalState::new(10, 2);
    let second = TerminalState::new(10, 2);
    first.feed(b"first");
    assert_eq!(second.snapshot().cells[0].cell.c, ' ');
}
