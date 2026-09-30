//! Starred-view model (#104, #131): the starred track list's count plus the
//! shared track table's sort/selection. The tracks themselves come from the
//! backend ([`crate::library_api::LibraryDataSource::starred_tracks`]);
//! rendering stays in the app.
//!
//! [`StarredView::refresh`] compares the incoming ids before allocating, so a
//! stable list does no work per frame.

use crate::library_api::TrackInfo;
use crate::views::Ctx;
use crate::views::filter;
use crate::views::track_table::TrackTable;

/// Persistent Starred-view state: the shared track table plus the visible
/// track ids and their count, rebuilt from the context's tracks when they
/// change.
#[derive(Debug, Default)]
pub struct StarredView {
    /// The starred track table (sort + selection).
    pub table: TrackTable,
    /// The starred track ids the table shows, in library order.
    track_ids: Vec<u64>,
    /// Bumped whenever the list of starred tracks changes.
    revision: u64,
}

impl StarredView {
    /// Rebuilds the visible starred ids from the context's tracks, keeping only
    /// those matching the active search, bumping the revision when the list
    /// changed.
    ///
    /// The incoming ids are compared before any allocation, so an unchanged
    /// list costs nothing per frame.
    pub fn refresh(&mut self, cx: &Ctx) {
        let visible: Vec<u64> = cx
            .tracks
            .iter()
            .filter(|track| filter::track_kept(cx.search, track.id))
            .map(|track| track.id)
            .collect();
        let changed = visible.len() != self.track_ids.len()
            || visible
                .iter()
                .zip(&self.track_ids)
                .any(|(id, previous)| id != previous);
        if changed {
            self.track_ids = visible;
            self.revision += 1;
        }
    }

    /// The starred tracks to show, resolved from the full starred list `all`
    /// (which must be in the same order as the one passed to
    /// [`StarredView::refresh`]), so a frontend renders exactly the filtered
    /// rows.
    pub fn visible_tracks<'a>(&self, all: &[&'a TrackInfo]) -> Vec<&'a TrackInfo> {
        let mut wanted = self.track_ids.iter().peekable();
        let mut visible = Vec::with_capacity(self.track_ids.len());
        for track in all {
            if wanted.peek() == Some(&&track.id) {
                wanted.next();
                visible.push(*track);
            }
        }
        visible
    }

    /// The starred track ids, in library order.
    pub fn track_ids(&self) -> &[u64] {
        &self.track_ids
    }

    /// The number of starred tracks.
    pub fn count(&self) -> usize {
        self.track_ids.len()
    }

    /// Whether no tracks are starred.
    pub fn is_empty(&self) -> bool {
        self.track_ids.is_empty()
    }

    /// The header count text, e.g. `"5 starred"`.
    pub fn count_label(&self) -> String {
        format!("{} starred", self.track_ids.len())
    }

    /// The revision counter, bumped whenever the starred list changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::SearchEngine;

    fn track(id: u64, starred: bool) -> TrackInfo {
        TrackInfo {
            id,
            starred,
            ..TrackInfo::default()
        }
    }

    #[test]
    fn refresh_counts_the_starred_tracks_and_bumps_on_change() {
        let tracks = [track(1, true), track(2, true)];
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let mut view = StarredView::default();

        let before = view.revision();
        view.refresh(&Ctx::new(&refs, None));
        assert_eq!(view.count(), 2);
        assert_eq!(view.track_ids(), &[1, 2]);
        assert_eq!(view.count_label(), "2 starred");
        assert!(view.revision() > before, "first refresh bumps");

        let after = view.revision();
        view.refresh(&Ctx::new(&refs, None));
        assert_eq!(view.revision(), after, "stable list doesn't bump");

        let fewer = [refs[0]];
        view.refresh(&Ctx::new(&fewer, None));
        assert_eq!(view.count(), 1);
        assert!(view.revision() > after, "unstarring bumps");
    }

    /// Ticks `engine` until it stops being pending, polling like the UI does.
    fn settle(engine: &mut SearchEngine, tracks: &[TrackInfo], query: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            engine.tick(tracks, query);
            if !engine.is_pending() || std::time::Instant::now() > deadline {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn search_keeps_only_matching_starred_tracks_and_restores_on_clear() {
        let tracks = [
            TrackInfo {
                id: 1,
                title: "One".to_string(),
                starred: true,
                ..TrackInfo::default()
            },
            TrackInfo {
                id: 2,
                title: "Two".to_string(),
                starred: true,
                ..TrackInfo::default()
            },
        ];
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let mut engine = SearchEngine::new();
        settle(&mut engine, &tracks, "one");

        let mut view = StarredView::default();
        view.refresh(&Ctx::new(&refs, None).with_search(&engine));
        assert_eq!(view.track_ids(), &[1]);
        let visible = view.visible_tracks(&refs);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, 1);

        settle(&mut engine, &tracks, "");
        view.refresh(&Ctx::new(&refs, None).with_search(&engine));
        assert_eq!(view.track_ids(), &[1, 2]);
        assert_eq!(view.visible_tracks(&refs).len(), 2);
    }

    #[test]
    fn a_query_matching_no_starred_track_leaves_no_rows() {
        let tracks = [track(1, true), track(2, true)];
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let mut engine = SearchEngine::new();
        settle(&mut engine, &tracks, "zzzz");

        let mut view = StarredView::default();
        view.refresh(&Ctx::new(&refs, None).with_search(&engine));
        assert!(view.is_empty());
        assert!(view.visible_tracks(&refs).is_empty());
    }
}
