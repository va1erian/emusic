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
    /// The library revision the album list was built from; `None` when the
    /// backend provides no cheap signal or the list is stale.
    source_revision: Option<u64>,
    /// The sort the album list was last ordered by.
    source_sort: AlbumSort,
    /// The selection the selected-track ids were resolved for.
    source_selected: Option<AlbumKey>,
    /// Bumped when the album list (its content or order) changes, so a
    /// retained-mode frontend rebuilds only its tiles.
    list_revision: u64,
    /// Bumped when the selected album's tracks change, so a frontend rebuilds
    /// only the track list without re-uploading the grid's covers.
    selection_revision: u64,
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
            source_revision: None,
            source_sort: AlbumSort::default(),
            source_selected: None,
            list_revision: 0,
            selection_revision: 0,
        }
    }
}

impl AlbumGrid {
    /// Selects the album identified by `name`/`artist`, as a click on its
    /// tile would, so the view shows its tracks. Used when jumping here from
    /// an album name elsewhere (e.g. the now-playing panel).
    pub fn select_album(&mut self, name: impl Into<String>, artist: impl Into<String>) {
        self.selected = Some(AlbumKey {
            name: name.into(),
            artist: artist.into(),
        });
    }

    /// Rebuilds the album list (sorted per [`AlbumGrid::sort`]) and the
    /// selected album's tracks from the library snapshot in `cx`.
    ///
    /// Cheap to call every frame: the library-derived album list is rebuilt
    /// only when the library's [`LibraryDataSource::revision`] or the sort
    /// changed, and the selected tracks only when the library or selection
    /// changed. An unchanged snapshot costs a couple of counter comparisons.
    pub fn refresh(&mut self, cx: &Ctx) {
        let Some(library) = cx.library else {
            return;
        };
        let revision = library.revision();
        let library_changed = match revision {
            Some(revision) => self.source_revision != Some(revision),
            None => true,
        };
        let sort_changed = self.source_sort != self.sort;
        let selection_changed = self.source_selected != self.selected;

        if library_changed || sort_changed {
            self.source_revision = revision;
            self.source_sort = self.sort;
            let meta = catalog::album_meta(library);
            let mut albums: Vec<AlbumInfo> = library.albums().to_vec();
            albums.sort_by(|a, b| catalog::compare(a, b, self.sort, &meta));
            if albums != self.albums {
                self.albums = albums;
                self.list_revision += 1;
            }
        }

        // The selected album's tracks, in disc/track order, so the (shared)
        // track table can be built from them and played as a whole.
        if library_changed || selection_changed {
            self.source_selected = self.selected.clone();
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
                self.selection_revision += 1;
            }
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

    /// The revision counter for the album list, bumped when its content or
    /// order changes (a sort change, or a new library snapshot).
    pub fn list_revision(&self) -> u64 {
        self.list_revision
    }

    /// The revision counter for the selected album's tracks, bumped when the
    /// selection or the library changes.
    pub fn selection_revision(&self) -> u64 {
        self.selection_revision
    }

    /// Applies one user intent, queueing any resulting commands. `cx` must
    /// carry the library snapshot (see [`Ctx::with_library`]).
    pub fn update(&mut self, msg: AlbumGridMsg, cx: &Ctx, out: &mut Commands) {
        match msg {
            AlbumGridMsg::TileClicked(key) => {
                if self.selected.as_ref() != Some(&key) {
                    self.selected = Some(key);
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
                }
            }
            AlbumGridMsg::SetTileSize(size) => {
                let size = size.clamp(MIN_TILE_SIZE, MAX_TILE_SIZE);
                if (self.tile_size - size).abs() > f32::EPSILON {
                    self.tile_size = size;
                }
            }
            AlbumGridMsg::CloseAlbum => {
                self.selected.take();
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
mod tests;
