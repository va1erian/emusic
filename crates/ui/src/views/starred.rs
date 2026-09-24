//! Starred-view model (#104, #131): the starred track list's count plus the
//! shared track table's sort/selection. The tracks themselves come from the
//! backend ([`crate::library_api::LibraryDataSource::starred_tracks`]);
//! rendering stays in the frontends.
//!
//! [`StarredView::refresh`] compares the incoming ids before allocating, so a
//! stable list does no work per frame.

use crate::views::Ctx;
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
    /// Rebuilds the visible starred ids from the context's (already filtered)
    /// tracks, bumping the revision when the list changed.
    ///
    /// The incoming ids are compared before any allocation, so an unchanged
    /// list costs nothing per frame.
    pub fn refresh(&mut self, cx: &Ctx) {
        let changed = cx.tracks.len() != self.track_ids.len()
            || cx
                .tracks
                .iter()
                .zip(&self.track_ids)
                .any(|(track, id)| track.id != *id);
        if changed {
            self.track_ids = cx.tracks.iter().map(|track| track.id).collect();
            self.revision += 1;
        }
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
    use crate::library_api::TrackInfo;

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
}
