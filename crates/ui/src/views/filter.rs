//! Shared top-bar search filtering for the views (#22).
//!
//! Every view that shows library tracks or names funnels the active top-bar
//! query through these helpers, so the same case/diacritic folding and
//! substring rules apply everywhere and an inactive (or absent) query keeps
//! everything. The track rule reuses the [`SearchEngine`]'s already-computed
//! match set; the name rule reuses the search popup's normalized-substring
//! rule, so `Café` matches `cafe`.

use crate::library_api::TrackInfo;
use crate::search::SearchEngine;

/// Whether the track `track_id` survives `search`. No search, or an inactive
/// query, keeps every track.
pub fn track_kept(search: Option<&SearchEngine>, track_id: u64) -> bool {
    search.is_none_or(|search| !search.is_active() || search.is_match(track_id))
}

/// Whether the free-text `name` (artist, album, genre, ...) matches `search`.
///
/// Uses the same lower-casing and diacritic folding as the track matcher, and
/// the same normalized-substring rule as the search popup. No search, or an
/// inactive query, keeps every name.
pub fn name_kept(search: Option<&SearchEngine>, name: &str) -> bool {
    let Some(search) = search.filter(|search| search.is_active()) else {
        return true;
    };
    let needle = emusic_search::normalize_text(search.query());
    // A whitespace-only query parses to no terms, so the engine matches every
    // track; keep every name too.
    if needle.trim().is_empty() {
        return true;
    }
    emusic_search::normalize_text(name).contains(&needle)
}

/// The tracks of `tracks` that survive `search`, in order.
pub fn kept_tracks<'a>(
    search: Option<&SearchEngine>,
    tracks: &[&'a TrackInfo],
) -> Vec<&'a TrackInfo> {
    tracks
        .iter()
        .copied()
        .filter(|track| track_kept(search, track.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::library_api::TrackInfo;

    fn track(id: u64, title: &str, artist: &str) -> TrackInfo {
        TrackInfo {
            id,
            title: title.to_string(),
            artist: artist.to_string(),
            ..TrackInfo::default()
        }
    }

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

    #[test]
    fn no_search_keeps_every_track_and_name() {
        assert!(track_kept(None, 1));
        assert!(name_kept(None, "Anything"));
        let tracks = [track(1, "A", "B")];
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        assert_eq!(kept_tracks(None, &refs).len(), 1);
    }

    #[test]
    fn active_search_drops_non_matching_tracks_and_clearing_restores_them() {
        let tracks = [
            track(1, "Homework", "Daft Punk"),
            track(2, "OK Computer", "Radiohead"),
        ];
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let mut engine = SearchEngine::new();
        settle(&mut engine, &tracks, "radiohead");

        let kept = kept_tracks(Some(&engine), &refs);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, 2);

        settle(&mut engine, &tracks, "");
        assert_eq!(kept_tracks(Some(&engine), &refs).len(), 2);
    }

    #[test]
    fn a_query_matching_nothing_yields_no_tracks() {
        let tracks = [track(1, "Homework", "Daft Punk")];
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let mut engine = SearchEngine::new();
        settle(&mut engine, &tracks, "zzzz");

        assert!(kept_tracks(Some(&engine), &refs).is_empty());
    }

    #[test]
    fn name_matching_is_case_and_diacritic_folded() {
        let tracks = [track(1, "x", "Beyoncé")];
        let mut engine = SearchEngine::new();
        settle(&mut engine, &tracks, "BEYONCE");

        assert!(name_kept(Some(&engine), "Beyoncé"));
        assert!(!name_kept(Some(&engine), "Radiohead"));
    }

    #[test]
    fn name_matching_keeps_everything_when_inactive() {
        let engine = SearchEngine::new();
        assert!(name_kept(Some(&engine), "Anything"));
    }
}
