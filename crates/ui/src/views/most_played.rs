//! Most-Played-view state (#24, #97): the selected time window plus the
//! shared track table's sort/selection. The ranking itself comes from the
//! backend; rendering stays in the egui frontend.

use crate::library_api::StatsWindow;
use crate::views::track_table::TrackTableState;

/// Persistent Most Played state: the selected window plus the track table's
/// sort/selection.
#[derive(Debug)]
pub struct MostPlayedState {
    pub window: StatsWindow,
    pub table: TrackTableState,
}

impl Default for MostPlayedState {
    fn default() -> Self {
        Self {
            window: StatsWindow::AllTime,
            table: TrackTableState::default(),
        }
    }
}
