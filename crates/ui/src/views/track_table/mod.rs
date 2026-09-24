//! Track-table view model: column definitions, sorting and per-instance
//! state (#15, #93, #97).
//!
//! Rendering (cells, stars, playing marker) stays in the frontends; this
//! module holds the column identities/widths/text, the sort order built from
//! header clicks, and the persistent sort/selection state each embedding view
//! owns.

pub mod columns;
pub mod selection;
pub mod sort;

use crate::library_api::TrackInfo;

use selection::SelectionState;
use sort::SortState;

/// Persistent per-instance state (sort order + selection). Each embedding
/// view owns one of these across frames.
#[derive(Debug, Default)]
pub struct TrackTableState {
    pub sort: SortState,
    pub selection: SelectionState,
    /// The track whose Properties dialog is open, if any. Owned here (rather
    /// than by the shell) so each embedding table gets its own dialog; the
    /// dialog is rendered by the frontend's table widget itself.
    pub properties: Option<TrackInfo>,
}
