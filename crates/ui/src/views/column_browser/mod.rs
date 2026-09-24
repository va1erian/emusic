//! Column-browser view model (#16, #93, #97).
//!
//! The persistent state (visibility, splitter height and per-pane
//! selections) and the track-matching rule live here; the pane widgets and
//! rendering stay in the frontends. The facet lists are built by the
//! frontends from the library snapshot.

pub mod selection;

use crate::library_api::TrackInfo;

use selection::PaneSelection;

/// Default splitter height, in pixels (the pane strip above the table),
/// sized so roughly eight to ten rows are visible in each pane.
pub const DEFAULT_HEIGHT: f32 = 200.0;
/// Smallest the column browser may be dragged to.
pub const MIN_HEIGHT: f32 = 80.0;
/// Largest the column browser may be dragged to.
pub const MAX_HEIGHT: f32 = 480.0;

/// Persistent column-browser state: visibility, splitter height and the
/// selection of each pane.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnBrowserState {
    pub visible: bool,
    pub height: f32,
    pub genres: PaneSelection,
    pub artists: PaneSelection,
    pub albums: PaneSelection,
}

impl Default for ColumnBrowserState {
    fn default() -> Self {
        Self {
            visible: true,
            height: DEFAULT_HEIGHT,
            genres: PaneSelection::default(),
            artists: PaneSelection::default(),
            albums: PaneSelection::default(),
        }
    }
}

impl ColumnBrowserState {
    /// Whether `track` passes all three panes' filters.
    pub fn matches(&self, track: &TrackInfo) -> bool {
        self.genres.matches(&track.genre)
            && self.artists.matches(&track.artist)
            && self.albums.matches(&track.album)
    }
}
