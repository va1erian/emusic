//! Multi-select state for the track table: click, ctrl+click, shift+click,
//! and arrow-key navigation. Selection is keyed by track id (not row index)
//! so it survives re-sorting and filtering.

use std::collections::HashSet;

/// Which modifier was held for a row click, decoupled from `egui::Modifiers`
/// so selection logic stays independent of the UI layer and is easy to
/// unit-test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClickModifiers {
    pub ctrl: bool,
    pub shift: bool,
}

/// Current selection, tracked by track id plus a "focused" row (the one
/// keyboard navigation and Enter act on) and an anchor for shift-click
/// ranges.
#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    selected: HashSet<u64>,
    /// Index (into the current sort order) of the shift-click range anchor.
    anchor: Option<usize>,
    /// Index (into the current sort order) of the keyboard-focused row.
    pub focus: Option<usize>,
}

impl SelectionState {
    pub fn is_selected(&self, track_id: u64) -> bool {
        self.selected.contains(&track_id)
    }

    pub fn selected_ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.selected.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Handles a click on the row at `index` (position in the current sort
    /// order), whose track id is `track_id`. `order` is the full current
    /// sort order, needed to resolve shift-click ranges.
    pub fn click(&mut self, order: &[u64], index: usize, track_id: u64, modifiers: ClickModifiers) {
        if modifiers.shift {
            let anchor = self.anchor.unwrap_or(index);
            let (lo, hi) = (anchor.min(index), anchor.max(index));
            if !modifiers.ctrl {
                self.selected.clear();
            }
            self.selected.extend(order[lo..=hi].iter().copied());
        } else if modifiers.ctrl {
            if !self.selected.insert(track_id) {
                self.selected.remove(&track_id);
            }
            self.anchor = Some(index);
        } else {
            self.selected.clear();
            self.selected.insert(track_id);
            self.anchor = Some(index);
        }
        self.focus = Some(index);
    }

    /// Moves keyboard focus by `delta` rows (clamped to `len`), replacing
    /// the selection with the newly focused row - standard list-box
    /// behaviour for plain arrow-key navigation.
    pub fn move_focus(&mut self, order: &[u64], delta: isize, len: usize) {
        if len == 0 {
            return;
        }
        let current = self.focus.unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, len as isize - 1) as usize;
        self.focus = Some(next);
        self.anchor = Some(next);
        self.selected.clear();
        self.selected.insert(order[next]);
    }

    /// Drops ids that no longer exist (e.g. after a search filter changes
    /// the visible set), keeping selection consistent.
    pub fn retain_existing(&mut self, existing: &HashSet<u64>) {
        self.selected.retain(|id| existing.contains(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> Vec<u64> {
        vec![10, 20, 30, 40, 50]
    }

    #[test]
    fn plain_click_selects_single_row() {
        let mut sel = SelectionState::default();
        sel.click(&order(), 2, 30, ClickModifiers::default());
        assert!(sel.is_selected(30));
        assert_eq!(sel.len(), 1);

        sel.click(&order(), 0, 10, ClickModifiers::default());
        assert!(sel.is_selected(10));
        assert!(!sel.is_selected(30));
    }

    #[test]
    fn ctrl_click_toggles_without_clearing() {
        let mut sel = SelectionState::default();
        sel.click(&order(), 0, 10, ClickModifiers::default());
        sel.click(
            &order(),
            2,
            30,
            ClickModifiers {
                ctrl: true,
                shift: false,
            },
        );
        assert!(sel.is_selected(10));
        assert!(sel.is_selected(30));
        assert_eq!(sel.len(), 2);

        // Ctrl-click again on an already-selected row deselects just it.
        sel.click(
            &order(),
            0,
            10,
            ClickModifiers {
                ctrl: true,
                shift: false,
            },
        );
        assert!(!sel.is_selected(10));
        assert!(sel.is_selected(30));
    }

    #[test]
    fn shift_click_selects_contiguous_range() {
        let mut sel = SelectionState::default();
        sel.click(&order(), 1, 20, ClickModifiers::default());
        sel.click(
            &order(),
            3,
            40,
            ClickModifiers {
                ctrl: false,
                shift: true,
            },
        );
        for id in [20, 30, 40] {
            assert!(sel.is_selected(id), "{id} should be in range");
        }
        assert!(!sel.is_selected(10));
        assert!(!sel.is_selected(50));
        assert_eq!(sel.len(), 3);
    }

    #[test]
    fn arrow_navigation_moves_and_replaces_selection() {
        let mut sel = SelectionState::default();
        sel.click(&order(), 1, 20, ClickModifiers::default());
        sel.move_focus(&order(), 1, order().len());
        assert_eq!(sel.focus, Some(2));
        assert!(sel.is_selected(30));
        assert!(!sel.is_selected(20));

        // Clamped at the ends.
        for _ in 0..10 {
            sel.move_focus(&order(), 1, order().len());
        }
        assert_eq!(sel.focus, Some(4));
    }

    #[test]
    fn retain_existing_drops_stale_ids() {
        let mut sel = SelectionState::default();
        sel.click(&order(), 0, 10, ClickModifiers::default());
        sel.click(
            &order(),
            1,
            20,
            ClickModifiers {
                ctrl: true,
                shift: false,
            },
        );
        let still_here: HashSet<u64> = [10].into_iter().collect();
        sel.retain_existing(&still_here);
        assert!(sel.is_selected(10));
        assert!(!sel.is_selected(20));
    }
}
