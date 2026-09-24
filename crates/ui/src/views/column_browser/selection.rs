//! Selection model for a single column-browser pane.
//!
//! Each pane's selection is a set of values; the empty set means the
//! "All (N)" row is active. Values are kept as `String` so the model is
//! independent of the UI row type and easy to unit-test.

use std::collections::BTreeSet;

/// Multi-select state for one pane (Genre, Artist or Album).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneSelection {
    selected: BTreeSet<String>,
}

impl PaneSelection {
    /// True when the "All (N)" row is active, i.e. nothing is filtered.
    pub fn is_all(&self) -> bool {
        self.is_empty()
    }

    /// True when no values are selected.
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    pub fn contains(&self, value: &str) -> bool {
        self.selected.contains(value)
    }

    /// Whether a facet `value` passes this pane's filter.
    pub fn matches(&self, value: &str) -> bool {
        self.selected.is_empty() || self.selected.contains(value)
    }

    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Applies a click on `value`; `None` is the "All" row, which clears the
    /// selection. Without `ctrl` the click replaces the selection; with
    /// `ctrl` it toggles the value.
    pub fn click(&mut self, value: Option<&str>, ctrl: bool) {
        let Some(value) = value else {
            self.selected.clear();
            return;
        };
        if ctrl {
            if !self.selected.insert(value.to_string()) {
                self.selected.remove(value);
            }
        } else {
            self.selected.clear();
            self.selected.insert(value.to_string());
        }
    }

    /// Drops selected values no longer offered after a parent selection
    /// changed, so a pane can't keep filtering on a facet that is hidden.
    pub fn retain(&mut self, available: &BTreeSet<String>) {
        self.selected.retain(|value| available.contains(value));
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the selection model, moved with the code (#93).

    use std::collections::BTreeSet;

    use super::PaneSelection;

    fn available(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|v| (*v).to_string()).collect()
    }

    #[test]
    fn empty_selection_means_all() {
        let mut selection = PaneSelection::default();
        assert!(selection.is_all());
        assert!(selection.matches("anything"));

        selection.click(Some("Rock"), false);
        assert!(!selection.is_all());
        assert!(selection.matches("Rock"));
        assert!(!selection.matches("Jazz"));
    }

    #[test]
    fn plain_click_replaces_and_ctrl_click_toggles() {
        let mut selection = PaneSelection::default();
        selection.click(Some("Rock"), false);
        selection.click(Some("Jazz"), false);
        assert_eq!(selection.len(), 1);
        assert!(selection.contains("Jazz"));
        assert!(!selection.contains("Rock"));

        // Ctrl adds a second value...
        selection.click(Some("Rock"), true);
        assert_eq!(selection.len(), 2);

        // ...and ctrl-clicking an existing value removes it again.
        selection.click(Some("Rock"), true);
        assert_eq!(selection.len(), 1);
        assert!(selection.contains("Jazz"));
    }

    #[test]
    fn all_row_clears_the_selection() {
        let mut selection = PaneSelection::default();
        selection.click(Some("Rock"), true);
        selection.click(Some("Jazz"), true);
        assert_eq!(selection.len(), 2);

        selection.click(None, false);
        assert!(selection.is_all());
    }

    #[test]
    fn retain_drops_values_the_parent_no_longer_offers() {
        let mut selection = PaneSelection::default();
        selection.click(Some("Rock"), false);
        selection.click(Some("Jazz"), true);

        selection.retain(&available(&["Jazz"]));
        assert!(selection.contains("Jazz"));
        assert!(!selection.contains("Rock"));
    }
}
