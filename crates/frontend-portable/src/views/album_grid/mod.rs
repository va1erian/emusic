//! The Albums view (#113), ported to the portable widget layer.
//!
//! All state and logic live in the shared [`AlbumGrid`] model (`emusic-ui`):
//! sorting, identity, the album list and the selected album's track table.
//! This module owns the virtual [`GridView`], the selected album's track list
//! and the view layout, and turns their events into [`AlbumMsg`]s. The toolbar
//! lives in [`toolbar`].
//!
//! The tile painter draws covers through the portable
//! [`Canvas`](xui::xui_core::backend::Canvas), so the same view renders on the
//! Win32 and canvas backends.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use emusic_ui::library_api::{LibraryDataSource, TrackInfo};
use emusic_ui::search::SearchEngine;
use emusic_ui::shell::Changes;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::album_grid::models::{AlbumKey, AlbumSort};
use emusic_ui::views::album_grid::{self, AlbumGrid, AlbumGridMsg, DEFAULT_TILE_SIZE};
use emusic_ui::views::{Commands, Ctx};
use emusic_ui::waker::WakerHandle;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::Result;
use xui::xui_core::geometry::{Point, Rect};
use xui::xui_core::units::dip;
use xui::xui_core::widget::GridView;

use crate::app::Msg;
use crate::views::track_table::TrackView;

use self::thumbs::ThumbState;
use self::tile::{AlbumCell, TileModel, content, tile_size};
use self::toolbar::Toolbar;

mod thumbs;
mod tile;
mod toolbar;

/// Gap between the toolbar, the grid and the track list, in design units.
const BAND_GAP: f32 = 4.0;

/// Everything the Albums view's controls can ask the app to do.
pub enum AlbumMsg {
    /// A tile was clicked; select that album.
    Select(usize),
    /// A tile was double-clicked; play the album.
    Activate(usize),
    /// The sort combo changed.
    SetSort(AlbumSort),
    /// The tile-size slider moved.
    SetTileSize(f32),
    /// The "Shuffle" button was clicked; shuffle the selected album.
    Shuffle,
    /// The "Close album" button was clicked.
    CloseAlbum,
}

/// The Albums view: the sort/size toolbar, the album grid and the selected
/// album's track list.
pub struct AlbumGridView {
    ui: Ui<Msg>,
    toolbar: Toolbar,
    grid: GridView<Msg>,
    tracks: TrackView,
    /// The albums currently in the grid, shared with the tile painter so it
    /// can resolve a tile index to its caption and cover source.
    cells: Rc<RefCell<Rc<Vec<AlbumCell>>>>,
    /// The shared cover cache the painter reads.
    thumbs: Rc<RefCell<ThumbState>>,
    /// The waker a rebuilt cover cache hands its workers.
    waker: WakerHandle,
    /// The album-list revision the grid model was last built from.
    grid_revision: Cell<u64>,
    /// The tile size last applied to the grid.
    applied_tile_size: Cell<f32>,
    /// The selection revision the track list was last built from.
    applied_selection_revision: Cell<u64>,
    /// Whether an album is selected (drives the close button and shuffle).
    selected: Cell<bool>,
    /// Whether the track list is currently shown.
    tracks_visible: Cell<bool>,
    /// Whether the Albums view is the active central view.
    active: Cell<bool>,
}

impl AlbumGridView {
    /// Creates the toolbar, the (empty) grid and the (empty) track list.
    /// `waker` lets the cover decoders repaint the grid when one finishes.
    pub fn new(ui: &mut Ui<Msg>, waker: WakerHandle) -> Result<Self> {
        let cells = Rc::new(RefCell::new(Rc::new(Vec::new())));
        let thumbs = Rc::new(RefCell::new(ThumbState::new(waker.clone())));

        let grid = GridView::with_model(
            ui,
            Rect::default(),
            TileModel {
                cells: Rc::clone(&cells.borrow()),
            },
        )?
        .tile_size(tile_size(DEFAULT_TILE_SIZE))
        .on_select(|index| Some(Msg::Album(AlbumMsg::Select(index))))
        .on_activate(|index| Some(Msg::Album(AlbumMsg::Activate(index))))
        .on_paint_tile(content(Rc::clone(&cells), Rc::clone(&thumbs)));

        let view = Self {
            ui: ui.clone(),
            toolbar: Toolbar::new(ui)?,
            grid,
            tracks: TrackView::new(ui),
            cells,
            thumbs,
            waker,
            grid_revision: Cell::new(u64::MAX),
            applied_tile_size: Cell::new(DEFAULT_TILE_SIZE),
            applied_selection_revision: Cell::new(u64::MAX),
            selected: Cell::new(false),
            tracks_visible: Cell::new(false),
            active: Cell::new(true),
        };
        view.apply_visibility();
        Ok(view)
    }

    /// Shows or hides the whole view (central-area routing).
    ///
    /// Hiding drops the cover cache: browsing another view must not keep a
    /// gallery's decoded artwork resident. The cover cache's LRU is bounded,
    /// but a hidden view's covers are dead weight until it is shown again.
    pub fn set_visible(&self, visible: bool) {
        if self.active.get() != visible {
            self.active.set(visible);
            if !visible {
                self.release_covers();
            }
            self.apply_visibility();
        }
    }

    /// Drops the decoded covers and the grid's uploaded-image handles. The
    /// portable [`Canvas`](xui::xui_core::backend::Canvas) has no per-image
    /// release hook, but the Win32 backend keeps its uploaded bitmaps in a
    /// byte-bounded LRU and evicts the least recently used covers on its own.
    /// Clearing the cache here releases the decoded RGBA and the `Rc<Image>`
    /// handles, so showing the view again decodes only the tiles it paints.
    fn release_covers(&self) {
        *self.thumbs.borrow_mut() = ThumbState::new(self.waker.clone());
    }

    /// Moves and sizes the toolbar, the grid and the selected album's track
    /// list inside `bounds`. The track list only takes the lower half when an
    /// album is selected.
    pub fn set_bounds(&self, bounds: Rect) {
        let content = self.toolbar.set_bounds(bounds);
        let gap = dip(BAND_GAP).to_px(self.ui.dpi()).value();
        let split = self.active.get() && self.tracks_visible.get() && content.height() > 0;
        let (grid, tracks) = if split {
            let middle = content.top + content.height() / 2;
            (
                Rect::new(content.left, content.top, content.right, middle),
                Rect::new(content.left, middle + gap, content.right, content.bottom),
            )
        } else {
            (content, Rect::default())
        };
        self.ui.apply_moves(&[(self.grid.id(), grid)]);
        self.tracks.set_bounds(tracks);
    }

    /// Refreshes the shared model and mirrors it into the grid and the track
    /// list. Returns whether the layout must be recomputed (the track list was
    /// shown or hidden).
    ///
    /// `changes` lets the track list be rebuilt when the library changed even
    /// though the selected album did not (e.g. a track was starred elsewhere),
    /// so its star glyphs stay current.
    pub fn sync(
        &mut self,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        search: &SearchEngine,
        playing_id: Option<u64>,
        changes: Changes,
    ) -> bool {
        let uploaded = self.thumbs.borrow_mut().drain();
        {
            let cx = Ctx::with_library(&[], playing_id, library).with_search(search);
            state.album_grid.refresh(&cx);
        }

        self.toolbar.sync(&state.album_grid);
        let selected = state.album_grid.selected_key().is_some();
        let selection_changed = self.selected.replace(selected) != selected;

        let list_revision = state.album_grid.list_revision();
        if list_revision != self.grid_revision.get() {
            self.rebuild_grid(&state.album_grid, library);
            self.grid_revision.set(list_revision);
        }
        if (self.applied_tile_size.get() - state.album_grid.tile_size).abs() > f32::EPSILON {
            self.grid
                .set_tile_size(tile_size(state.album_grid.tile_size));
            self.applied_tile_size.set(state.album_grid.tile_size);
        }

        let selected_index = state
            .album_grid
            .selected_key()
            .and_then(|key| self.cell_index(key));
        if self.grid.selected() != selected_index {
            self.grid.select(selected_index);
        }

        let selection_revision = state.album_grid.selection_revision();
        let stale = changes.intersects(Changes::LIBRARY)
            || selection_revision != self.applied_selection_revision.get();
        let tracks_changed = if stale {
            self.applied_selection_revision.set(selection_revision);
            let tracks = self.selected_tracks(&state.album_grid, library);
            self.tracks.set_rows(&tracks, state.album_grid.table.sort);
            self.tracks.sync_playing(playing_id);
            let visible = !tracks.is_empty();
            self.tracks_visible.replace(visible) != visible
        } else {
            self.tracks.sync_playing(playing_id);
            false
        };

        if selection_changed || tracks_changed {
            self.apply_visibility();
        }
        if uploaded {
            self.ui.invalidate(self.grid.id());
        }
        tracks_changed
    }

    /// Applies one control event, queueing any resulting commands.
    pub fn update(
        &mut self,
        msg: AlbumMsg,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        playing_id: Option<u64>,
        out: &mut Commands,
    ) {
        let cx = Ctx::with_library(&[], playing_id, library);
        match msg {
            AlbumMsg::Select(index) => {
                if let Some(key) = self.key_at(index) {
                    state
                        .album_grid
                        .update(AlbumGridMsg::TileClicked(key), &cx, out);
                }
            }
            AlbumMsg::Activate(index) => {
                if let Some(key) = self.key_at(index) {
                    state
                        .album_grid
                        .update(AlbumGridMsg::TileActivated(key), &cx, out);
                }
            }
            AlbumMsg::SetSort(sort) => {
                state
                    .album_grid
                    .update(AlbumGridMsg::SetSort(sort), &cx, out);
            }
            AlbumMsg::SetTileSize(size) => {
                state
                    .album_grid
                    .update(AlbumGridMsg::SetTileSize(size), &cx, out);
            }
            AlbumMsg::Shuffle => {
                if let Some(key) = state.album_grid.selected_key().cloned() {
                    state
                        .album_grid
                        .update(AlbumGridMsg::Shuffle(key), &cx, out);
                }
            }
            AlbumMsg::CloseAlbum => {
                state.album_grid.update(AlbumGridMsg::CloseAlbum, &cx, out);
            }
        }
    }

    /// Rebuilds the track list after a header click, preserving the new sort.
    pub fn resort(&mut self, state: &AppState, library: &dyn LibraryDataSource) {
        let tracks = self.selected_tracks(&state.album_grid, library);
        self.tracks.set_rows(&tracks, state.album_grid.table.sort);
    }

    /// The command to play `index` in the context of the selected album.
    pub fn activate(&self, index: usize) -> Option<Command> {
        self.tracks.activate(index)
    }

    /// The track at `index`, for a context action.
    pub fn track(&self, index: usize) -> Option<TrackInfo> {
        self.tracks.track(index).cloned()
    }

    /// The track list's client origin, to anchor a row context menu.
    pub fn context_origin(&self) -> Point {
        self.tracks.context_origin()
    }

    /// Pushes each control's visibility from the active/selection state, so a
    /// hidden view never shows a stray control.
    fn apply_visibility(&self) {
        let active = self.active.get();
        self.toolbar.set_visible(active, self.selected.get());
        self.ui.set_visible(self.grid.id(), active);
        self.tracks.set_visible(active && self.tracks_visible.get());
    }

    /// Rebuilds the grid model from the album list, resolving each album's
    /// cover source from the shared catalog.
    fn rebuild_grid(&mut self, grid: &AlbumGrid, library: &dyn LibraryDataSource) {
        let meta = album_grid::album_meta(library);
        let cells: Vec<AlbumCell> = grid
            .albums()
            .iter()
            .map(|album| {
                let key = AlbumKey::of(album);
                let art_path = meta
                    .get(&key)
                    .map(|meta| meta.art_path.clone())
                    .unwrap_or_default();
                AlbumCell {
                    key,
                    name: album.name.clone(),
                    artist: album.artist.clone(),
                    year: album.year,
                    art_path,
                }
            })
            .collect();
        let cells = Rc::new(cells);
        *self.cells.borrow_mut() = Rc::clone(&cells);
        self.grid.set_model(TileModel { cells });
    }

    /// The selected album's tracks, resolved from the library snapshot.
    fn selected_tracks<'a>(
        &self,
        grid: &AlbumGrid,
        library: &'a dyn LibraryDataSource,
    ) -> Vec<&'a TrackInfo> {
        grid.selected_track_ids()
            .iter()
            .filter_map(|id| library.tracks().iter().find(|track| track.id == *id))
            .collect()
    }

    /// The album key at grid position `index`.
    fn key_at(&self, index: usize) -> Option<AlbumKey> {
        self.cells.borrow().get(index).map(|cell| cell.key.clone())
    }

    /// The grid index of `key`, if it is in the current album list.
    fn cell_index(&self, key: &AlbumKey) -> Option<usize> {
        self.cells.borrow().iter().position(|cell| &cell.key == key)
    }
}
