//! Album-grid view model (#17, #93).
//!
//! Identity, ordering and track matching — the parts that don't draw.
//! The grid state, thumbnail cache and tile painters stay in the frontends.

pub mod catalog;
pub mod models;

use crate::views::track_table::TrackTableState;

use models::{AlbumKey, AlbumSort};

/// Default album tile edge length, in pixels.
pub const DEFAULT_TILE_SIZE: f32 = 148.0;

/// Persistent album-grid state, stored on the shell's `AppState`.
///
/// The egui frontend keeps its cover-texture cache separately (it owns GPU
/// handles), so this stays toolkit-agnostic.
#[derive(Debug)]
pub struct AlbumGridState {
    /// Cover edge length, driven by the size slider.
    pub tile_size: f32,
    /// Current sort order.
    pub sort: AlbumSort,
    /// Selected album, whose tracks are shown below the grid.
    pub selected: Option<AlbumKey>,
    /// The selected album's track table (sort + selection).
    pub table: TrackTableState,
}

impl Default for AlbumGridState {
    fn default() -> Self {
        Self {
            tile_size: DEFAULT_TILE_SIZE,
            sort: AlbumSort::default(),
            selected: None,
            table: TrackTableState::default(),
        }
    }
}

impl AlbumGridState {
    /// Selects the album identified by `name`/`artist`, as a click on its
    /// tile would, so the view shows its tracks. Used when jumping here from
    /// an album name elsewhere (e.g. the now-playing panel).
    pub fn select_album(&mut self, name: impl Into<String>, artist: impl Into<String>) {
        self.selected = Some(AlbumKey {
            name: name.into(),
            artist: artist.into(),
        });
    }
}
