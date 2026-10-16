use latch::sftp::Entry;
use std::{cmp::Ordering, path::Path};

#[cfg(test)]
mod sort_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui) enum Side {
    Local,
    Remote,
}

impl Side {
    pub(in crate::ui) fn id(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::ui) enum Column {
    #[default]
    Name,
    Modified,
    Size,
    Type,
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::ui) struct Sort {
    pub column: Column,
    pub descending: bool,
}

impl Sort {
    pub(in crate::ui) fn toggle(&mut self, column: Column) {
        self.descending = self.column == column && !self.descending;
        self.column = column;
    }

    pub(in crate::ui) fn compare(self, left: &Entry, right: &Entry) -> Ordering {
        if left.is_dir != right.is_dir {
            return right.is_dir.cmp(&left.is_dir);
        }
        let order = match self.column {
            Column::Name => names(left, right),
            Column::Modified => left.modified.cmp(&right.modified),
            Column::Size => file_size(left).cmp(&file_size(right)),
            Column::Type => entry_type(left)
                .to_lowercase()
                .cmp(&entry_type(right).to_lowercase()),
        }
        .then_with(|| names(left, right));
        if self.descending {
            order.reverse()
        } else {
            order
        }
    }
}

fn names(left: &Entry, right: &Entry) -> Ordering {
    left.name
        .to_lowercase()
        .cmp(&right.name.to_lowercase())
        .then_with(|| left.name.cmp(&right.name))
}

pub(super) fn file_size(entry: &Entry) -> Option<u64> {
    if entry.is_dir { None } else { entry.size }
}

pub(super) fn entry_type(entry: &Entry) -> &str {
    if entry.is_link {
        "Link"
    } else if entry.is_dir {
        "Folder"
    } else {
        Path::new(&entry.name)
            .extension()
            .and_then(|extension| extension.to_str())
            .filter(|extension| !extension.is_empty())
            .unwrap_or("File")
    }
}

#[derive(Debug)]
pub(in crate::ui) struct History<T> {
    paths: Vec<T>,
    index: usize,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            index: 0,
        }
    }
}

impl<T: Clone + PartialEq> History<T> {
    pub(in crate::ui) fn commit(&mut self, path: T) {
        if self.paths.get(self.index) == Some(&path) {
            return;
        }
        self.paths.truncate(self.index + 1);
        self.paths.push(path);
        self.index = self.paths.len() - 1;
    }

    pub(in crate::ui) fn commit_at(&mut self, index: usize, path: T) {
        if let Some(previous) = self.paths.get_mut(index) {
            *previous = path;
            self.index = index;
        } else {
            self.commit(path);
        }
    }

    #[cfg(test)]
    pub(in crate::ui) fn can_back(&self) -> bool {
        self.index > 0
    }

    #[cfg(test)]
    pub(in crate::ui) fn can_forward(&self) -> bool {
        self.index + 1 < self.paths.len()
    }

    pub(in crate::ui) fn target_back(&self) -> Option<(usize, T)> {
        let index = self.index.checked_sub(1)?;
        Some((index, self.paths.get(index)?.clone()))
    }

    pub(in crate::ui) fn target_forward(&self) -> Option<(usize, T)> {
        let index = self.index + 1;
        Some((index, self.paths.get(index)?.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, size: u64) -> Entry {
        Entry {
            name: name.into(),
            path: name.into(),
            is_dir: false,
            is_link: false,
            size: Some(size),
            modified: Some(size),
        }
    }

    #[test]
    fn sort_toggles_direction_and_resets_on_new_column() {
        let mut sort = Sort::default();
        sort.toggle(Column::Name);
        assert!(sort.descending);
        sort.toggle(Column::Size);
        assert!(!sort.descending);
    }

    #[test]
    fn size_sort_compares_numbers_instead_of_display_text() {
        let sort = Sort {
            column: Column::Size,
            descending: false,
        };
        assert_eq!(
            sort.compare(&entry("a", 20), &entry("b", 100)),
            Ordering::Less
        );
    }

    #[test]
    fn folders_never_display_or_sort_by_their_reported_size() {
        let mut folder = entry("folder", 100);
        folder.is_dir = true;
        assert_eq!(file_size(&folder), None);
    }

    #[test]
    fn link_type_takes_precedence_over_directory_and_extension() {
        let mut link = entry("linked.txt", 0);
        link.is_link = true;
        link.is_dir = true;
        assert_eq!(entry_type(&link), "Link");
    }

    #[test]
    fn history_moves_only_after_success_and_keeps_forward_paths() {
        let mut history = History::default();
        history.commit("a");
        history.commit("b");
        let target = history.target_back().unwrap();
        assert_eq!(history.target_back(), Some((0, "a")));
        history.commit_at(target.0, target.1);
        assert_eq!(history.target_forward(), Some((1, "b")));
    }

    #[test]
    fn new_navigation_after_back_discards_forward_history() {
        let mut history = History::default();
        history.commit("a");
        history.commit("b");
        history.commit_at(0, "a");
        history.commit("c");
        assert_eq!(history.target_forward(), None);
    }
}
