use serde::{Deserialize, Serialize};

use crate::TrackId;

/// A recorded playback of a track, used to derive play counts and history.
///
/// `played_at` and `duration_played_ms` are stored so that stats (most
/// played, recently played, "scrobble"-style thresholds) can be computed
/// without re-deriving them from raw player events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayEvent {
    /// The track that was played.
    pub track_id: TrackId,
    /// Unix timestamp (seconds, UTC) when playback started.
    pub played_at: i64,
    /// How much of the track was actually played, in milliseconds.
    pub duration_played_ms: u32,
}

impl PlayEvent {
    /// Creates a new play event.
    pub fn new(track_id: TrackId, played_at: i64, duration_played_ms: u32) -> Self {
        Self {
            track_id,
            played_at,
            duration_played_ms,
        }
    }

    /// Returns `true` if the played duration meets or exceeds `threshold_ms`,
    /// the common heuristic for counting a play as a "real" listen.
    pub fn counts_as_play(&self, threshold_ms: u32) -> bool {
        self.duration_played_ms >= threshold_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_as_play_respects_threshold() {
        let event = PlayEvent::new(TrackId(1), 1_700_000_000, 30_000);
        assert!(event.counts_as_play(30_000));
        assert!(event.counts_as_play(15_000));
        assert!(!event.counts_as_play(30_001));
    }
}
