use std::sync::{Arc, Mutex, MutexGuard};

use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::{Config, TermMode, cell::Cell, color::Colors},
    vte::ansi::{CursorShape, Processor},
};

#[derive(Clone)]
/// Shared terminal state. Feed remote bytes from a background I/O task.
pub struct TerminalState(Arc<Mutex<Inner>>);

struct Inner {
    term: Term<Replies>,
    parser: Processor,
    replies: Arc<Mutex<Vec<u8>>>,
    revision: u64,
}

#[derive(Clone)]
struct Replies(Arc<Mutex<Vec<u8>>>);

impl EventListener for Replies {
    fn send_event(&self, event: Event) {
        // Remote clipboard escape sequences must never read or overwrite the local clipboard.
        if let Event::PtyWrite(text) = event {
            lock(&self.0).extend_from_slice(text.as_bytes());
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(super) struct Snapshot {
    pub cells: Vec<VisibleCell>,
    pub colors: Colors,
    pub cursor: (usize, usize, CursorShape),
    pub rows: usize,
}

pub(super) struct VisibleCell {
    pub row: usize,
    pub column: usize,
    pub cell: Cell,
    pub selected: bool,
}

struct Size {
    columns: usize,
    rows: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

impl TerminalState {
    /// Create an independent terminal with 10,000 lines of scrollback.
    pub fn new(columns: u16, rows: u16) -> Self {
        let replies = Arc::new(Mutex::new(Vec::new()));
        let size = Size {
            columns: usize::from(columns.max(2)),
            rows: usize::from(rows.max(1)),
        };
        let config = Config {
            scrolling_history: 10_000,
            ..Config::default()
        };
        Self(Arc::new(Mutex::new(Inner {
            term: Term::new(config, &size, Replies(replies.clone())),
            parser: Processor::new(),
            replies,
            revision: 0,
        })))
    }

    /// Parse remote output on the I/O task; send returned protocol replies back to SSH.
    pub fn feed(&self, bytes: &[u8]) -> Vec<u8> {
        let mut inner = lock(&self.0);
        let Inner { parser, term, .. } = &mut *inner;
        parser.advance(term, bytes);
        inner.revision = inner.revision.wrapping_add(1);
        std::mem::take(&mut *lock(&inner.replies))
    }

    /// Resize the grid, returning whether the remote PTY also needs resizing.
    pub fn resize(&self, columns: u16, rows: u16) -> bool {
        let mut inner = lock(&self.0);
        let size = Size {
            columns: usize::from(columns.max(2)),
            rows: usize::from(rows.max(1)),
        };
        if inner.term.columns() == size.columns && inner.term.screen_lines() == size.rows {
            return false;
        }
        inner.term.resize(size);
        inner.revision = inner.revision.wrapping_add(1);
        true
    }

    pub(super) fn revision(&self) -> u64 {
        lock(&self.0).revision
    }

    pub(super) fn mode(&self) -> TermMode {
        *lock(&self.0).term.mode()
    }

    pub(super) fn scroll(&self, lines: i32) {
        let mut inner = lock(&self.0);
        inner.term.scroll_display(Scroll::Delta(lines));
        inner.revision = inner.revision.wrapping_add(1);
    }

    pub(super) fn scroll_bottom(&self) {
        let mut inner = lock(&self.0);
        inner.term.scroll_display(Scroll::Bottom);
        inner.term.selection = None;
    }

    pub(super) fn select(&self, row: usize, column: usize, start: bool) {
        let mut inner = lock(&self.0);
        let term = &mut inner.term;
        let row = row.min(term.screen_lines() - 1) as i32 - term.grid().display_offset() as i32;
        let point = Point::new(Line(row), Column(column.min(term.columns() - 1)));
        if start {
            term.selection = Some(Selection::new(SelectionType::Simple, point, Side::Left));
        } else if let Some(selection) = &mut term.selection {
            selection.update(point, Side::Right);
            selection.include_all();
        }
        inner.revision = inner.revision.wrapping_add(1);
    }

    pub(super) fn selected_text(&self) -> Option<String> {
        lock(&self.0).term.selection_to_string()
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        let inner = lock(&self.0);
        let content = inner.term.renderable_content();
        let offset = content.display_offset as i32;
        let cursor = content.cursor;
        let cells = content
            .display_iter
            .map(|indexed| VisibleCell {
                row: (indexed.point.line.0 + offset) as usize,
                column: indexed.point.column.0,
                selected: content
                    .selection
                    .is_some_and(|range| range.contains(indexed.point)),
                cell: indexed.cell.clone(),
            })
            .collect();
        Snapshot {
            cells,
            colors: *content.colors,
            rows: inner.term.screen_lines(),
            cursor: (
                (cursor.point.line.0 + offset) as usize,
                cursor.point.column.0,
                cursor.shape,
            ),
        }
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
