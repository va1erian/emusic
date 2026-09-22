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
