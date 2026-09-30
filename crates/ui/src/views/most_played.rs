//! Most-Played-view state (#24, #97): the selected time window plus the
//! shared track table's sort/selection. The ranking itself comes from the
//! backend; rendering stays in the app.

use crate::library_api::{StatsWindow, TrackInfo};
use crate::search::SearchEngine;
use crate::views::filter;
use crate::views::track_table::TrackTable;

/// Persistent Most Played state: the selected window plus the track table's
/// sort/selection.
#[derive(Debug)]
pub struct MostPlayedState {
    pub window: StatsWindow,
    pub table: TrackTable,
}

impl Default for MostPlayedState {
    fn default() -> Self {
        Self {
            window: StatsWindow::AllTime,
            table: TrackTable::default(),
        }
    }
}

impl MostPlayedState {
    /// The ranked tracks that survive the active top-bar search, in ranking
    /// order.
    pub fn visible_tracks<'a>(
        &self,
        ranked: &[&'a TrackInfo],
        search: Option<&SearchEngine>,
    ) -> Vec<&'a TrackInfo> {
        filter::kept_tracks(search, ranked)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    /// Ticks `engine` until it stops being pending, polling like the UI does.
    fn settle(engine: &mut SearchEngine, tracks: &[TrackInfo], query: &str) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            engine.tick(tracks, query);
            if !engine.is_pending() || Instant::now() > deadline {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn track(id: u64, title: &str) -> TrackInfo {
        TrackInfo {
            id,
            title: title.to_string(),
            ..TrackInfo::default()
        }
    }

    #[test]
    fn visible_tracks_keeps_only_matches_and_restores_on_clear() {
        let ranked = [track(1, "One"), track(2, "Two")];
        let refs: Vec<&TrackInfo> = ranked.iter().collect();
        let state = MostPlayedState::default();
        let mut engine = SearchEngine::new();

        settle(&mut engine, &ranked, "one");
        let visible = state.visible_tracks(&refs, Some(&engine));
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, 1);

        settle(&mut engine, &ranked, "zzzz");
        assert!(state.visible_tracks(&refs, Some(&engine)).is_empty());

        settle(&mut engine, &ranked, "");
        assert_eq!(state.visible_tracks(&refs, Some(&engine)).len(), 2);
    }
}
