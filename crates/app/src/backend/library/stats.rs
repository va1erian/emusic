//! Stats helpers for the library backend.
//!
//! Aggregated per-track play stats live in the store; this module loads them
//! for a snapshot. Recording a finished play is handled by the library's
//! [`StatsRecorder`](emusic_library::stats::StatsRecorder) so the database
//! write never blocks the UI thread.

use std::collections::HashMap;

use emusic_library::{Store, Track, TrackId, TrackStats};

/// Loads aggregated stats for `tracks` in one pass.
///
/// Only tracks that have been played or skipped have a `track_stats` row, so
/// the returned map is usually much smaller than `tracks`.
pub(crate) fn load_all(
    store: &Store,
    tracks: &[Track],
) -> anyhow::Result<HashMap<TrackId, TrackStats>> {
    let mut map = HashMap::new();
    for track in tracks {
        if let Some(stats) = store.track_stats(track.id)? {
            map.insert(track.id, stats);
        }
    }
    Ok(map)
}
