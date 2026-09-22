//! Read-side stats queries: paged history and "most played" windows.
//!
//! These are thin wrappers over [`crate::Store`]'s stats primitives that
//! add the bits an actual UI needs (page-to-offset math, a window-to-cutoff
//! mapping) so callers work in pages and named windows rather than raw
//! `LIMIT`/`OFFSET` and Unix timestamps.

use crate::error::Result;
use crate::store::{MostPlayedEntry, PlayHistoryEntry, Store};

/// A time window for "most played" rankings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsWindow {
    AllTime,
    Last30Days,
    LastYear,
}

impl StatsWindow {
    /// Every window, in display order (used by the "most played" selector).
    pub const ALL: [Self; 3] = [Self::AllTime, Self::Last30Days, Self::LastYear];

    /// Short, human-readable label for the window.
    pub fn label(self) -> &'static str {
        match self {
            Self::AllTime => "All time",
            Self::Last30Days => "Last 30 days",
            Self::LastYear => "Last year",
        }
    }

    /// The number of seconds this window spans, or `None` for all time.
    fn span_secs(self) -> Option<i64> {
        const DAY: i64 = 24 * 60 * 60;
        match self {
            StatsWindow::AllTime => None,
            StatsWindow::Last30Days => Some(30 * DAY),
            StatsWindow::LastYear => Some(365 * DAY),
        }
    }

    /// The cutoff timestamp (plays at or after this count), given the
    /// current time `now` as a Unix timestamp.
    fn since(self, now: i64) -> Option<i64> {
        self.span_secs().map(|span| now - span)
    }
}

/// Returns page `page` (0-indexed) of playback history, newest first.
pub fn history_page(store: &Store, page: u32, page_size: u32) -> Result<Vec<PlayHistoryEntry>> {
    let page_size = page_size.max(1);
    let offset = page.saturating_mul(page_size);
    store.play_history_page(page_size, offset)
}

/// Returns the most-played tracks within `window`, ranked by completed play
/// count, highest first. `now` is the current time (a Unix timestamp);
/// passed in rather than read internally so this stays testable without
/// mocking the clock.
pub fn most_played(
    store: &Store,
    window: StatsWindow,
    limit: u32,
    now: i64,
) -> Result<Vec<MostPlayedEntry>> {
    store.most_played_since(window.since(now), limit)
}

#[cfg(test)]
mod tests;
