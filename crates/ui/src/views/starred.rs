//! Starred-view model (#104, #131): the starred track list's count plus the
//! shared track table's sort/selection. The tracks themselves come from the
//! backend ([`LibraryDataSource::starred_tracks`]); rendering stays in the
//! frontends.
//!
//! [`LibraryDataSource::starred_tracks`]:
//!     crate::library_api::LibraryDataSource::starred_tracks

use crate::views::track_table::{TrackTable, TrackTableMsg};
use crate::views::{Commands, Ctx};

/// A user intent on the Starred view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarredMsg {
    /// The starred track list changed its sort/selection.
    Table(TrackTableMsg),
}

/// Persistent Starred-view state: the shared track table plus the visible
/// track ids and their count, rebuilt each frame.
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
    pub fn refresh(&mut self, cx: &Ctx) {
        let track_ids: Vec<u64> = cx.tracks.iter().map(|track| track.id).collect();
        if track_ids != self.track_ids {
            self.track_ids = track_ids;
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

    /// Applies one user intent, queueing any resulting commands.
    pub fn update(&mut self, msg: StarredMsg, cx: &Ctx, out: &mut Commands) {
        match msg {
            StarredMsg::Table(msg) => self.table.update(msg, cx, out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_api::TrackInfo;
    use crate::state::Command;
    use crate::views::track_table::{ContextAction, TrackTableMsg};

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

    #[test]
    fn table_messages_are_delegated() {
        let tracks = [track(1, true)];
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let cx = Ctx::new(&refs, None);
        let mut view = StarredView::default();
        view.refresh(&cx);
        view.table.refresh(&cx);

        let mut out = Commands::new();
        view.update(
            StarredMsg::Table(TrackTableMsg::Context {
                row: 0,
                action: ContextAction::ToggleStar,
            }),
            &cx,
            &mut out,
        );
        assert_eq!(out.into_vec(), vec![Command::ToggleStarred(1)]);
    }
}
