//! Album-grid view model (#17, #93, #100).
//!
//! Identity, ordering, track matching and the scroll-independent grid layout
//! live here, so the grid virtualizes the same way. The tile painters
//! and the thumbnail cache stay in the app (the cache owns GPU/OS
//! handles, #96).

pub mod catalog;
pub mod models;

use crate::library_api::AlbumInfo;
use crate::state::Command;
use crate::views::track_table::{TrackTable, TrackTableMsg};
use crate::views::{Commands, Ctx};

use models::{AlbumKey, AlbumSort};

/// Default album tile edge length, in pixels.
pub const DEFAULT_TILE_SIZE: f32 = 148.0;

/// Tile edge-length bounds for the size slider, in pixels.
pub const MIN_TILE_SIZE: f32 = 96.0;
/// Maximum cover edge length selected with the size slider.
pub const MAX_TILE_SIZE: f32 = 256.0;

/// A user intent on the album grid.
#[derive(Debug, Clone, PartialEq)]
pub enum AlbumGridMsg {
    /// A tile was clicked; select that album.
    TileClicked(AlbumKey),
    /// A tile was double-clicked (or otherwise activated); play its tracks.
    TileActivated(AlbumKey),
    /// The selected album's tracks changed their sort/selection.
    Table(TrackTableMsg),
    /// Change the grid's sort mode.
    SetSort(AlbumSort),
    /// Change the cover edge length.
    SetTileSize(f32),
    /// Close the selected album, returning to the full grid.
    CloseAlbum,
    /// Start a shuffled playback over the album's tracks.
    Shuffle(AlbumKey),
}

/// Read-only display data for one tile.
#[derive(Debug, Clone, Copy)]
pub struct TileView<'a> {
    /// The tile's position in the current sort order (row-major).
    pub index: usize,
    /// The album shown.
    pub album: &'a AlbumInfo,
    /// Whether the tile is the selected album.
    pub selected: bool,
}

/// Persistent album-grid state plus the display data derived from the library
/// snapshot each frame.
#[derive(Debug)]
pub struct AlbumGrid {
    /// Cover edge length, driven by the size slider.
    pub tile_size: f32,
    /// Current sort order.
    pub sort: AlbumSort,
    /// Selected album, whose tracks are shown below the grid.
    pub selected: Option<AlbumKey>,
    /// The selected album's track table (sort + selection).
    pub table: TrackTable,
    /// Albums in display order, rebuilt by [`AlbumGrid::refresh`].
    albums: Vec<AlbumInfo>,
    /// The selected album's track ids, in disc/track order.
    selected_track_ids: Vec<u64>,
    /// Bumped whenever what the view displays changes.
    revision: u64,
}

impl Default for AlbumGrid {
    fn default() -> Self {
        Self {
            tile_size: DEFAULT_TILE_SIZE,
            sort: AlbumSort::default(),
            selected: None,
            table: TrackTable::default(),
            albums: Vec::new(),
            selected_track_ids: Vec::new(),
            revision: 0,
        }
    }
}

impl AlbumGrid {
    /// Selects the album identified by `name`/`artist`, as a click on its
    /// tile would, so the view shows its tracks. Used when jumping here from
    /// an album name elsewhere (e.g. the now-playing panel).
    pub fn select_album(&mut self, name: impl Into<String>, artist: impl Into<String>) {
        self.revision += 1;
        self.selected = Some(AlbumKey {
            name: name.into(),
            artist: artist.into(),
        });
    }

    /// Rebuilds the album list (sorted per [`AlbumGrid::sort`]) and the
    /// selected album's tracks from the library snapshot in `cx`.
    pub fn refresh(&mut self, cx: &Ctx) {
        let Some(library) = cx.library else {
            return;
        };
        let meta = catalog::album_meta(library);
        let mut albums: Vec<AlbumInfo> = library.albums().to_vec();
        albums.sort_by(|a, b| catalog::compare(a, b, self.sort, &meta));
        if albums != self.albums {
            self.albums = albums;
            self.revision += 1;
        }

        // The selected album's tracks, in disc/track order, so the (shared)
        // track table can be built from them and played as a whole.
        let selected_ids = self
            .selected_key()
            .and_then(|key| self.albums.iter().find(|album| AlbumKey::of(album) == *key))
            .map(|album| {
                catalog::album_tracks(library, album)
                    .into_iter()
                    .map(|track| track.id)
                    .collect::<Vec<u64>>()
            })
            .unwrap_or_default();
        if selected_ids != self.selected_track_ids {
            self.selected_track_ids = selected_ids;
            self.revision += 1;
        }
    }

    /// The album list in display order.
    pub fn albums(&self) -> &[AlbumInfo] {
        &self.albums
    }

    /// The number of albums shown.
    pub fn len(&self) -> usize {
        self.albums.len()
    }

    /// Whether the grid is empty.
    pub fn is_empty(&self) -> bool {
        self.albums.is_empty()
    }

    /// The display data for tile `i`, or `None` when out of range.
    pub fn tile(&self, index: usize) -> Option<TileView<'_>> {
        let album = self.albums.get(index)?;
        Some(TileView {
            index,
            album,
            selected: self.selected.as_ref() == Some(&AlbumKey::of(album)),
        })
    }

    /// The selected album's key, if any.
    pub fn selected_key(&self) -> Option<&AlbumKey> {
        self.selected.as_ref()
    }

    /// The selected album's track ids, in disc/track order.
    pub fn selected_track_ids(&self) -> &[u64] {
        &self.selected_track_ids
    }

    /// Number of grid columns for a viewport `width`, given the item spacing.
    /// Shared so the grid virtualizes identically.
    pub fn columns_for(&self, width: f32, spacing: f32) -> usize {
        (((width + spacing) / (self.tile_size + spacing)).floor() as usize).max(1)
    }

    /// The number of rows the grid lays out for `columns` columns.
    pub fn rows(&self, columns: usize) -> usize {
        self.albums.len().div_ceil(columns.max(1))
    }

    /// The tile index at grid position `row`/`column`, if it exists.
    pub fn tile_index(&self, row: usize, column: usize, columns: usize) -> Option<usize> {
        let index = row.checked_mul(columns)?.checked_add(column)?;
        (index < self.albums.len()).then_some(index)
    }

    /// The revision counter, bumped whenever the displayed state changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Applies one user intent, queueing any resulting commands. `cx` must
    /// carry the library snapshot (see [`Ctx::with_library`]).
    pub fn update(&mut self, msg: AlbumGridMsg, cx: &Ctx, out: &mut Commands) {
        match msg {
            AlbumGridMsg::TileClicked(key) => {
                if self.selected.as_ref() != Some(&key) {
                    self.selected = Some(key);
                    self.revision += 1;
                }
            }
            AlbumGridMsg::TileActivated(key) => {
                let ids = self.album_ids(&key, cx);
                if !ids.is_empty() {
                    out.push(Command::PlayAlbum(ids));
                }
            }
            AlbumGridMsg::Table(msg) => self.table.update(msg, cx, out),
            AlbumGridMsg::SetSort(sort) => {
                if self.sort != sort {
                    self.sort = sort;
                    self.revision += 1;
                }
            }
            AlbumGridMsg::SetTileSize(size) => {
                let size = size.clamp(MIN_TILE_SIZE, MAX_TILE_SIZE);
                if (self.tile_size - size).abs() > f32::EPSILON {
                    self.tile_size = size;
                    self.revision += 1;
                }
            }
            AlbumGridMsg::CloseAlbum => {
                if self.selected.take().is_some() {
                    self.revision += 1;
                }
            }
            AlbumGridMsg::Shuffle(key) => {
                let ids = self.album_ids(&key, cx);
                if !ids.is_empty() {
                    out.push(Command::ShuffleScope {
                        ids,
                        label: key.name,
                    });
                }
            }
        }
    }

    /// The track ids of the album `key`, resolved from the library snapshot
    /// when present, else from the cached selected-album tracks.
    fn album_ids(&self, key: &AlbumKey, cx: &Ctx) -> Vec<u64> {
        if let Some(library) = cx.library
            && let Some(album) = self.albums.iter().find(|album| AlbumKey::of(album) == *key)
        {
            return catalog::album_tracks(library, album)
                .into_iter()
                .map(|track| track.id)
                .collect();
        }
        if self.selected.as_ref() == Some(key) {
            self.selected_track_ids.clone()
        } else {
            Vec::new()
        }
    }
}

/// Re-exported for callers that need the album's tracks outside the model
/// (e.g. the now-playing panel's links).
pub use catalog::{album_meta, album_tracks};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockLibrary;

    fn grid_with_library(library: &dyn crate::library_api::LibraryDataSource) -> AlbumGrid {
        let mut grid = AlbumGrid::default();
        grid.refresh(&Ctx::with_library(&[], None, library));
        grid
    }

    #[test]
    fn refresh_populates_albums_and_sorts() {
        let library = MockLibrary::new();
        let source: &dyn crate::library_api::LibraryDataSource = &library;
        let grid = grid_with_library(source);
        assert_eq!(grid.len(), source.albums().len());
        // Default sort is by artist, case-insensitively.
        let artists: Vec<&str> = grid
            .albums()
            .iter()
            .map(|album| album.artist.as_str())
            .collect();
        let mut expected = artists.clone();
        expected.sort_by_key(|a| a.to_lowercase());
        assert_eq!(artists, expected);
    }

    #[test]
    fn columns_for_matches_the_grid_layout_math() {
        let library = MockLibrary::new();
        let mut grid = grid_with_library(&library);
        grid.tile_size = 100.0;
        // (width + spacing) / (tile + spacing), floored.
        assert_eq!(grid.columns_for(430.0, 10.0), 4);
        assert_eq!(grid.columns_for(0.0, 10.0), 1, "always at least one column");
    }

    #[test]
    fn tile_index_is_row_major_and_bounded() {
        let library = MockLibrary::new();
        let grid = grid_with_library(&library);
        let columns = 3;
        assert_eq!(grid.tile_index(0, 0, columns), Some(0));
        assert_eq!(grid.tile_index(0, 1, columns), Some(1));
        assert_eq!(grid.tile_index(1, 0, columns), Some(3));
        assert_eq!(grid.tile_index(usize::MAX, 0, columns), None);
    }

    #[test]
    fn tile_clicked_selects_and_close_clears() {
        let library = MockLibrary::new();
        let mut grid = grid_with_library(&library);
        let key = AlbumKey::of(&grid.albums()[0]);
        let mut out = Commands::new();
        grid.update(
            AlbumGridMsg::TileClicked(key.clone()),
            &Ctx::with_library(&[], None, &library),
            &mut out,
        );
        assert_eq!(grid.selected_key(), Some(&key));
        assert!(grid.tile(0).is_some_and(|view| view.selected));

        grid.update(
            AlbumGridMsg::CloseAlbum,
            &Ctx::with_library(&[], None, &library),
            &mut out,
        );
        assert!(grid.selected_key().is_none());
    }

    #[test]
    fn tile_activated_plays_the_albums_tracks() {
        let library = MockLibrary::new();
        let mut grid = grid_with_library(&library);
        let album = grid.albums()[0].clone();
        let expected: Vec<u64> = album_tracks(&library, &album)
            .into_iter()
            .map(|track| track.id)
            .collect();

        let mut out = Commands::new();
        grid.update(
            AlbumGridMsg::TileActivated(AlbumKey::of(&album)),
            &Ctx::with_library(&[], None, &library),
            &mut out,
        );
        assert_eq!(out.into_vec(), vec![Command::PlayAlbum(expected)]);
    }

    #[test]
    fn revision_bumps_on_sort_and_tile_size_changes() {
        let library = MockLibrary::new();
        let mut grid = grid_with_library(&library);
        let cx = Ctx::with_library(&[], None, &library);
        let before = grid.revision();
        grid.update(
            AlbumGridMsg::SetSort(AlbumSort::Year),
            &cx,
            &mut Commands::new(),
        );
        assert!(grid.revision() > before);
        let after_sort = grid.revision();
        grid.update(
            AlbumGridMsg::SetTileSize(grid.tile_size),
            &cx,
            &mut Commands::new(),
        );
        assert_eq!(grid.revision(), after_sort, "same size doesn't bump");
    }
}
