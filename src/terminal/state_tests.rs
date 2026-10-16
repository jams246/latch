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
fn reconnect_should_restore_shell_history_and_default_modes() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"first\r\nsecond\r\nshell\x1b[?1049happ\x1b[?1h\x1b[?2004h\x1b[?25l");
    terminal.reset_for_reconnect();
    assert_eq!(terminal.mode(), TermMode::default());
    assert_eq!(terminal.snapshot().cells[0].cell.c, 's');
    terminal.scroll(1);
    assert_eq!(terminal.snapshot().cells[0].cell.c, 'f');
}

#[test]
fn reconnect_should_discard_partial_escape_sequences_and_pending_replies() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"old\x1b[31m\x1b[31");
    lock(&lock(&terminal.0).replies).extend_from_slice(b"stale");
    terminal.reset_for_reconnect();
    assert!(terminal.feed(b"X").is_empty());
    let cell = &terminal.snapshot().cells[3].cell;
    assert_eq!(
        (cell.c, cell.fg),
        ('X', Color::Named(NamedColor::Foreground))
    );
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

#[test]
fn remote_clipboard_requests_never_generate_replies() {
    let terminal = TerminalState::new(10, 2);
    assert!(
        terminal
            .feed(b"\x1b]52;c;?\x07\x1b]52;c;c2VjcmV0\x07")
            .is_empty()
    );
}

#[test]
fn combining_flood_stays_bounded_across_output_chunks() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"a");
    let marks = "\u{301}".repeat(1024);
    for _ in 0..32 {
        terminal.feed(marks.as_bytes());
    }
    terminal.feed(b"b");
    let snapshot = terminal.snapshot();
    assert_eq!(snapshot.cells[0].cell.zerowidth().unwrap().len(), 32);
    assert_eq!(snapshot.cells[1].cell.c, 'b');
}

#[test]
fn ordinary_combining_characters_remain_visible() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed("a\u{301}\u{308}".as_bytes());
    assert_eq!(
        terminal.snapshot().cells[0].cell.zerowidth(),
        Some(['\u{301}', '\u{308}'].as_slice())
    );
}

#[test]
fn oversized_remote_color_command_is_ignored_and_parser_recovers() {
    let terminal = TerminalState::new(10, 2);
    terminal.feed(b"\x1b]10;");
    for _ in 0..32 {
        terminal.feed(&[b'x'; 1024]);
    }
    terminal.feed(b"\x07ok\x1b[6n");
    let snapshot = terminal.snapshot();
    assert_eq!(snapshot.colors[NamedColor::Foreground], None);
    assert_eq!(snapshot.cells[0].cell.c, 'o');
    assert_eq!(snapshot.cells[1].cell.c, 'k');
}
