//! History-view state (#24, #97): the shared table's selection plus the
//! "clear history" confirmation flag. Rendering stays in the egui frontend.

use crate::views::track_table::selection::SelectionState;

/// Persistent History-view state.
#[derive(Debug, Default)]
pub struct HistoryState {
    /// Row selection and keyboard focus, keyed by history entry id.
    pub selection: SelectionState,
    /// Whether the "clear history" confirmation dialog is open.
    pub confirm_clear: bool,
}
