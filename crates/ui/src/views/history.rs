//! History-view state and grouping (#24, #97, #246): the shared table's
//! selection, the "clear history" confirmation flag, and the toolkit-agnostic
//! day grouping both frontends render.
//!
//! Grouping is deliberately timezone-free: a play belongs to "Today",
//! "Yesterday" or "N days ago" based on whole 24-hour periods before `now`,
//! which keeps the mock screenshots byte-stable and avoids pulling in a
//! calendar/timezone dependency. A local-midnight grouping is a follow-up.

use crate::library_api::HistoryEntry;
use crate::views::track_table::selection::SelectionState;

/// Persistent History-view state.
#[derive(Debug, Default)]
pub struct HistoryState {
    /// Row selection and keyboard focus, keyed by history entry id.
    pub selection: SelectionState,
    /// Whether the "clear history" confirmation dialog is open.
    pub confirm_clear: bool,
}

/// One row of the flattened history: a day header or a play entry.
#[derive(Debug)]
pub enum Row<'a> {
    Day(String),
    Entry(&'a HistoryEntry),
}

/// Flattens `entries` (normally newest first) into day headers + entry rows.
///
/// Entries are bucketed by day label and the buckets are emitted in
/// first-seen order; with the store's newest-first ordering that yields
/// contiguous, correctly ordered groups, and it also keeps a single header
/// per day if the input is not perfectly sorted.
pub fn build_rows(entries: &[HistoryEntry], now: i64) -> Vec<Row<'_>> {
    let mut groups: Vec<(String, Vec<&HistoryEntry>)> = Vec::new();
    for entry in entries {
        let day = day_label(entry.played_at, now);
        match groups.iter_mut().find(|(label, _)| *label == day) {
            Some((_, group)) => group.push(entry),
            None => groups.push((day, vec![entry])),
        }
    }

    let mut rows = Vec::with_capacity(entries.len() + groups.len());
    for (label, group) in groups {
        rows.push(Row::Day(label));
        rows.extend(group.into_iter().map(Row::Entry));
    }
    rows
}

/// "Today" / "Yesterday" / "N days ago" for when `played_at` was recorded.
pub fn day_label(played_at: i64, now: i64) -> String {
    let days = (now - played_at).max(0) / 86_400;
    match days {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        n => format!("{n} days ago"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64, played_at: i64) -> HistoryEntry {
        HistoryEntry {
            id,
            played_at,
            ..HistoryEntry::default()
        }
    }

    const DAY: i64 = 86_400;

    #[test]
    fn day_label_buckets_by_whole_days() {
        let now = 1_000_000_000;
        assert_eq!(day_label(now, now), "Today");
        assert_eq!(day_label(now - 3_600, now), "Today");
        assert_eq!(day_label(now - DAY, now), "Yesterday");
        assert_eq!(day_label(now - DAY - 1, now), "Yesterday");
        assert_eq!(day_label(now - 2 * DAY, now), "2 days ago");
        assert_eq!(day_label(now - 7 * DAY, now), "7 days ago");
        // A future/clock-skewed timestamp never goes negative.
        assert_eq!(day_label(now + 100, now), "Today");
    }

    #[test]
    fn build_rows_inserts_one_header_per_day_change() {
        let now = 1_000_000_000;
        let entries = vec![
            entry(1, now),
            entry(2, now - 60),
            entry(3, now - DAY),
            entry(4, now - DAY - 60),
            entry(5, now - 3 * DAY),
        ];
        let rows = build_rows(&entries, now);

        let labels: Vec<&str> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Day(label) => Some(label.as_str()),
                Row::Entry(_) => None,
            })
            .collect();
        assert_eq!(labels, vec!["Today", "Yesterday", "3 days ago"]);

        let entry_ids: Vec<i64> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Entry(entry) => Some(entry.id),
                Row::Day(_) => None,
            })
            .collect();
        assert_eq!(entry_ids, vec![1, 2, 3, 4, 5]);
        assert_eq!(rows.len(), 8);
    }

    #[test]
    fn empty_history_produces_no_rows() {
        assert!(build_rows(&[], 1_000).is_empty());
    }
}
