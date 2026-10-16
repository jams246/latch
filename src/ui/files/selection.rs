use gpui::Modifiers;
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct Selection {
    pub selected: BTreeSet<usize>,
    anchor: Option<usize>,
}

impl Selection {
    pub(super) fn click(&mut self, index: usize, ordered: &[usize], modifiers: Modifiers) {
        let Some(position) = ordered.iter().position(|&item| item == index) else {
            return;
        };
        if modifiers.shift
            && let Some(anchor) = self
                .anchor
                .and_then(|anchor| ordered.iter().position(|&item| item == anchor))
        {
            if !modifiers.control {
                self.selected.clear();
            }
            self.selected
                .extend(&ordered[anchor.min(position)..=anchor.max(position)]);
            return;
        }
        if modifiers.control {
            if !self.selected.remove(&index) {
                self.selected.insert(index);
            }
        } else {
            self.selected.clear();
            self.selected.insert(index);
        }
        self.anchor = Some(index);
    }

    pub(super) fn context(&mut self, index: usize) {
        if !self.selected.contains(&index) {
            self.selected.clear();
            self.selected.insert(index);
            self.anchor = Some(index);
        }
    }

    pub(super) fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_click_replaces_selection() {
        let mut selection = Selection::default();
        selection.click(0, &[0, 1], Modifiers::default());
        selection.click(1, &[0, 1], Modifiers::default());
        assert_eq!(selection.selected, BTreeSet::from([1]));
    }

    #[test]
    fn control_click_toggles_without_removing_other_items() {
        let mut selection = Selection::default();
        let control = Modifiers {
            control: true,
            ..Modifiers::default()
        };
        for index in [0, 1, 0] {
            selection.click(index, &[0, 1], control);
        }
        assert_eq!(selection.selected, BTreeSet::from([1]));
    }

    #[test]
    fn shift_range_uses_display_order_and_keeps_anchor() {
        let mut selection = Selection::default();
        let ordered = [3, 1, 4, 0, 2];
        selection.click(1, &ordered, Modifiers::default());
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        selection.click(2, &ordered, shift);
        assert_eq!(selection.selected, BTreeSet::from([0, 1, 2, 4]));
        selection.click(3, &ordered, shift);
        assert_eq!(selection.selected, BTreeSet::from([1, 3]));
    }

    #[test]
    fn control_shift_adds_a_range_to_existing_selection() {
        let mut selection = Selection::default();
        selection.click(0, &[0, 1, 2, 3], Modifiers::default());
        selection.click(
            2,
            &[0, 1, 2, 3],
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
        );
        selection.click(
            3,
            &[0, 1, 2, 3],
            Modifiers {
                control: true,
                shift: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(selection.selected, BTreeSet::from([0, 2, 3]));
    }

    #[test]
    fn context_preserves_selected_items_and_replaces_unselected_items() {
        let mut selection = Selection::default();
        selection.selected.extend([0, 1]);
        selection.context(1);
        assert_eq!(selection.selected, BTreeSet::from([0, 1]));
        selection.context(2);
        assert_eq!(selection.selected, BTreeSet::from([2]));
    }

    #[test]
    fn clearing_selection_clears_the_range_anchor() {
        let mut selection = Selection::default();
        selection.click(0, &[0, 1, 2], Modifiers::default());
        selection.clear();
        selection.click(
            2,
            &[0, 1, 2],
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(selection.selected, BTreeSet::from([2]));
    }

    #[test]
    fn sorting_preserves_indices_and_changes_only_the_next_display_range() {
        let mut selection = Selection::default();
        selection.click(0, &[0, 1, 2], Modifiers::default());
        selection.click(
            1,
            &[1, 2, 0],
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        assert_eq!(selection.selected, BTreeSet::from([0, 1, 2]));
    }
}
