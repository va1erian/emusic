//! Music-view model (#15, #16, #97, #99): the column browser and the track
//! table composed into the full-library view, plus the live search filter and
//! result count.
//!
//! [`MusicView`] owns both children and wraps their messages
//! ([`MusicMsg`]), routing each to the right child and queueing any
//! [`Command`]s they emit. Rendering stays in the frontends.
//!
//! The model stores the *indices* of the tracks it shows (into the library
//! snapshot the caller passes to [`MusicView::refresh`]), not the tracks
//! themselves, so the model stays free of borrows and owns no library data.

use crate::library_api::TrackInfo;
use crate::search::SearchEngine;
use crate::state::Command;
use crate::views::column_browser::{ColumnBrowser, ColumnBrowserMsg};
use crate::views::track_table::{TrackTable, TrackTableMsg};
use crate::views::{Commands, Ctx};

/// A user intent on the Music view, wrapping its children's messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicMsg {
    Browser(ColumnBrowserMsg),
    Table(TrackTableMsg),
    /// Start a shuffled playback over the whole library.
    ShuffleAll,
}

/// The Music view: the column browser above the library track table.
#[derive(Debug, Default)]
pub struct MusicView {
    pub browser: ColumnBrowser,
    pub table: TrackTable,
    /// Indices (into the last `refresh` slice) of the tracks the table shows.
    visible: Vec<usize>,
    /// Total number of tracks in the library, shown next to the shuffled
    /// playback button.
    total: usize,
    /// Number of tracks matching the current search query; `None` when no
    /// query is active. Read by the status bar.
    search_result_count: Option<usize>,
}

impl MusicView {
    /// Rebuilds the column browser from the library snapshot `tracks` and the
    /// search, recomputing which tracks the table shows.
    pub fn refresh(&mut self, tracks: &[&TrackInfo], search: &SearchEngine) {
        self.total = tracks.len();
        self.browser.refresh(&Ctx::new(tracks, None));
        self.visible = tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| self.browser.matches(track))
            .filter(|(_, track)| search.is_match(track.id))
            .map(|(index, _)| index)
            .collect();
        self.search_result_count = search.is_active().then_some(self.visible.len());
    }

    /// The tracks the table should show, as last computed by
    /// [`MusicView::refresh`], resolved against the same `tracks` slice.
    /// Callers pass the result to [`Ctx::new`] and to the renderer.
    pub fn visible_tracks<'a>(&self, tracks: &'a [&TrackInfo]) -> Vec<&'a TrackInfo> {
        self.visible
            .iter()
            .filter_map(|&index| tracks.get(index).copied())
            .collect()
    }

    /// Total number of tracks in the library.
    pub fn total(&self) -> usize {
        self.total
    }

    /// Number of tracks matching the active search query, or `None`.
    pub fn search_result_count(&self) -> Option<usize> {
        self.search_result_count
    }

    /// Applies one user intent, queueing any resulting commands. `cx` must be
    /// built from [`MusicView::visible_tracks`].
    pub fn update(&mut self, msg: MusicMsg, cx: &Ctx, out: &mut Commands) {
        match msg {
            MusicMsg::Browser(msg) => self.browser.update(msg),
            MusicMsg::Table(msg) => self.table.update(msg, cx, out),
            MusicMsg::ShuffleAll => {
                let ids: Vec<u64> = cx.tracks.iter().map(|track| track.id).collect();
                out.push(Command::ShuffleScope {
                    ids,
                    label: "Music".to_string(),
                });
            }
        }
    }

    /// The revision counter, bumped whenever anything the view displays
    /// changes.
    pub fn revision(&self) -> u64 {
        self.browser.revision().wrapping_add(self.table.revision())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: u64, genre: &str) -> TrackInfo {
        TrackInfo {
            id,
            genre: genre.to_string(),
            ..TrackInfo::default()
        }
    }

    #[test]
    fn refresh_filters_by_browser_selection() {
        let tracks = [track(1, "Rock"), track(2, "Jazz")];
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let search = SearchEngine::default();
        let mut view = MusicView::default();
        view.refresh(&refs, &search);
        assert_eq!(view.total(), 2);
        assert_eq!(view.visible_tracks(&refs).len(), 2);

        view.update(
            MusicMsg::Browser(ColumnBrowserMsg::RowClicked {
                pane: crate::views::column_browser::Pane::Genre,
                value: Some("Rock".to_string()),
                ctrl: false,
            }),
            &Ctx::new(&refs, None),
            &mut Commands::new(),
        );
        view.refresh(&refs, &search);
        let visible = view.visible_tracks(&refs);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, 1);
    }

    #[test]
    fn shuffle_all_emits_a_scoped_shuffle_over_the_visible_tracks() {
        let tracks = [track(1, "Rock"), track(2, "Jazz")];
        let refs: Vec<&TrackInfo> = tracks.iter().collect();
        let mut view = MusicView::default();
        view.refresh(&refs, &SearchEngine::default());

        let mut out = Commands::new();
        view.update(MusicMsg::ShuffleAll, &Ctx::new(&refs, None), &mut out);
        assert_eq!(
            out.into_vec(),
            vec![Command::ShuffleScope {
                ids: vec![1, 2],
                label: "Music".to_string(),
            }]
        );
    }
}
