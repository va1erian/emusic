//! Listening-time accounting for the current track.
//!
//! Kept as plain data (no clock reads inside), so [`crate::Player`] is the
//! only place that reads wall-clock time and this logic stays trivially
//! unit-testable.

use std::time::Duration;

/// A track counts as "completed" once listened time reaches half its
/// duration, capped at this — per the issue: `min(50% duration, 4 min)`.
const MAX_COMPLETION_THRESHOLD: Duration = Duration::from_secs(4 * 60);

/// Accumulates how long the current track has actually been audible
/// (playing, not paused/stopped/seeking-away-from).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ListenAccounting {
    listened: Duration,
}

impl ListenAccounting {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `delta` wall-clock time that was spent actually playing.
    pub fn add(&mut self, delta: Duration) {
        self.listened += delta;
    }

    pub fn listened(&self) -> Duration {
        self.listened
    }

    /// Whether the accumulated listened time meets the "real listen"
    /// threshold for a track of `track_duration` (`None` if unknown, in
    /// which case only the flat 4-minute cap applies).
    pub fn completed(&self, track_duration: Option<Duration>) -> bool {
        let threshold = match track_duration {
            Some(duration) => (duration / 2).min(MAX_COMPLETION_THRESHOLD),
            None => MAX_COMPLETION_THRESHOLD,
        };
        self.listened >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_track_uses_half_duration_threshold() {
        let mut a = ListenAccounting::new();
        a.add(Duration::from_secs(29));
        assert!(!a.completed(Some(Duration::from_secs(60))));
        a.add(Duration::from_secs(2));
        assert!(a.completed(Some(Duration::from_secs(60))));
    }

    #[test]
    fn long_track_caps_threshold_at_four_minutes() {
        let mut a = ListenAccounting::new();
        a.add(Duration::from_secs(3 * 60 + 59));
        // Half of a 20 minute track would be 10 minutes, but the cap is 4.
        assert!(!a.completed(Some(Duration::from_secs(20 * 60))));
        a.add(Duration::from_secs(1));
        assert!(a.completed(Some(Duration::from_secs(20 * 60))));
    }

    #[test]
    fn unknown_duration_falls_back_to_four_minute_cap() {
        let mut a = ListenAccounting::new();
        a.add(Duration::from_secs(3 * 60 + 59));
        assert!(!a.completed(None));
        a.add(Duration::from_secs(1));
        assert!(a.completed(None));
    }

    #[test]
    fn accumulates_across_multiple_adds() {
        let mut a = ListenAccounting::new();
        a.add(Duration::from_secs(10));
        a.add(Duration::from_secs(20));
        assert_eq!(a.listened(), Duration::from_secs(30));
    }
}
