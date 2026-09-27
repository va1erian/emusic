//! The Albums view (#113), ported to the portable widget layer.
//!
//! All state and logic live in the shared [`AlbumGrid`] model (`emusic-ui`):
//! sorting, identity, the album list and the selected album's track table.
//! This module owns the sort/size toolbar, the virtual [`GridView`] and the
//! selected album's track list, and turns their events into [`AlbumMsg`]s.
//!
//! The tile painter draws covers through the portable [`Canvas`](xui::xui_core::backend::Canvas),
//! so the same view renders on the Win32 and canvas backends.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use emusic_ui::library_api::{LibraryDataSource, TrackInfo};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::album_grid::models::{AlbumKey, AlbumSort};
use emusic_ui::views::album_grid::{
    self, AlbumGrid, AlbumGridMsg, DEFAULT_TILE_SIZE, MAX_TILE_SIZE, MIN_TILE_SIZE,
};
use emusic_ui::views::{Commands, Ctx};
use emusic_ui::waker::WakerHandle;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::Result;
use xui::xui_core::geometry::{Point, Rect};
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, ComboBox, GridView, HasText, Label, Slider, TileSize};

use crate::app::Msg;
use crate::views::track_table::TrackView;

use self::thumbs::ThumbState;
use self::tile::{AlbumCell, CAPTION_DIP, GAP_DIP, TileModel, content};

mod thumbs;
mod tile;

/// The toolbar band height, in design units.
const TOOLBAR_HEIGHT: f32 = 34.0;
/// Fixed toolbar control widths, in design units.
const COUNT_WIDTH: f32 = 92.0;
const SORT_LABEL_WIDTH: f32 = 34.0;
const SORT_WIDTH: f32 = 150.0;
const SIZE_LABEL_WIDTH: f32 = 34.0;
const SHUFFLE_WIDTH: f32 = 88.0;
const CLOSE_WIDTH: f32 = 104.0;
/// Toolbar horizontal gaps and the band inset, in design units.
const GAP: f32 = 6.0;
const INSET: f32 = 8.0;
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
    count: Label<Msg>,
    sort_label: Label<Msg>,
    sort: ComboBox<Msg>,
    size_label: Label<Msg>,
    size: Slider<Msg>,
    shuffle: Button<Msg>,
    close: Button<Msg>,
    grid: GridView<Msg>,
    tracks: TrackView,
    /// The albums currently in the grid, shared with the tile painter so it
    /// can resolve a tile index to its caption and cover source.
    cells: Rc<RefCell<Rc<Vec<AlbumCell>>>>,
    /// The shared cover cache the painter reads.
    thumbs: Rc<RefCell<ThumbState>>,
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
        let thumbs = Rc::new(RefCell::new(ThumbState::new(waker)));

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

        let sort = ComboBox::new(ui, Rect::default(), &sort_labels())?
            .on_select(|index| Some(Msg::Album(AlbumMsg::SetSort(AlbumSort::ALL[index]))));
        sort.select(sort_index(AlbumSort::default()));

        let size = Slider::new(
            ui,
            Rect::default(),
            f64::from(MIN_TILE_SIZE),
            f64::from(MAX_TILE_SIZE),
        )?
        .on_change(|value| Some(Msg::Album(AlbumMsg::SetTileSize(value as f32))));
        size.set_value(f64::from(DEFAULT_TILE_SIZE));

        let shuffle = Button::new(ui, Rect::default(), "Shuffle")?
            .on_click(|| Some(Msg::Album(AlbumMsg::Shuffle)));
        let close = Button::new(ui, Rect::default(), "Close album")?
            .on_click(|| Some(Msg::Album(AlbumMsg::CloseAlbum)));

        let view = Self {
            ui: ui.clone(),
            count: Label::new(ui, Rect::default(), "0 albums")?,
            sort_label: Label::new(ui, Rect::default(), "Sort")?,
            sort,
            size_label: Label::new(ui, Rect::default(), "Size")?,
            size,
            shuffle,
            close,
            grid,
            tracks: TrackView::new(ui),
            cells,
            thumbs,
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
    pub fn set_visible(&self, visible: bool) {
        if self.active.get() != visible {
            self.active.set(visible);
            self.apply_visibility();
        }
    }

    /// Moves and sizes the toolbar, the grid and the selected album's track
    /// list inside `bounds`. The track list only takes the lower half when an
    /// album is selected.
    pub fn set_bounds(&self, bounds: Rect) {
        let dpi = self.ui.dpi();
        let px = |value: f32| dip(value).to_px(dpi).value();
        let top = bounds.top;
        let band_right = bounds.right - px(INSET);
        let gap = px(GAP);

        let mut x = bounds.left + px(INSET);
        let count = Rect::new(x, top, x + px(COUNT_WIDTH), top + px(TOOLBAR_HEIGHT));
        x += px(COUNT_WIDTH) + gap;
        let sort_label = Rect::new(x, top, x + px(SORT_LABEL_WIDTH), top + px(TOOLBAR_HEIGHT));
        x += px(SORT_LABEL_WIDTH) + gap;
        let sort = Rect::new(x, top, x + px(SORT_WIDTH), top + px(TOOLBAR_HEIGHT));
        x += px(SORT_WIDTH) + gap;
        let size_label = Rect::new(x, top, x + px(SIZE_LABEL_WIDTH), top + px(TOOLBAR_HEIGHT));
        x += px(SIZE_LABEL_WIDTH) + gap;

        let close = Rect::new(
            band_right - px(CLOSE_WIDTH),
            top,
            band_right,
            top + px(TOOLBAR_HEIGHT),
        );
        let shuffle = Rect::new(
            close.left - gap - px(SHUFFLE_WIDTH),
            top,
            close.left - gap,
            top + px(TOOLBAR_HEIGHT),
        );
        let size = Rect::new(x, top, shuffle.left - gap, top + px(TOOLBAR_HEIGHT));

        self.ui.apply_moves(&[
            (self.count.id(), count),
            (self.sort_label.id(), sort_label),
            (self.sort.id(), sort),
            (self.size_label.id(), size_label),
            (self.size.id(), size),
            (self.shuffle.id(), shuffle),
            (self.close.id(), close),
        ]);

        let content = Rect::new(
            bounds.left,
            top + px(TOOLBAR_HEIGHT) + px(BAND_GAP),
            bounds.right,
            bounds.bottom,
        );
        let split = self.active.get() && self.tracks_visible.get() && content.height() > 0;
        let (grid, tracks) = if split {
            let middle = content.top + content.height() / 2;
            (
                Rect::new(content.left, content.top, content.right, middle),
                Rect::new(
                    content.left,
                    middle + px(BAND_GAP),
                    content.right,
                    content.bottom,
                ),
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
    pub fn sync(
        &mut self,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        playing_id: Option<u64>,
    ) -> bool {
        let uploaded = self.thumbs.borrow_mut().drain();
        {
            let cx = Ctx::with_library(&[], playing_id, library);
            state.album_grid.refresh(&cx);
        }

        self.count
            .set_text(&format!("{} albums", state.album_grid.len()));
        let index = sort_index(state.album_grid.sort);
        if self.sort.selected() != index {
            self.sort.select(index);
        }
        let size = f64::from(state.album_grid.tile_size);
        if (self.size.value() - size).abs() > f64::EPSILON {
            self.size.set_value(size);
        }
        let selected = state.album_grid.selected_key().is_some();
        self.shuffle.set_enabled(selected);
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
        let tracks_changed = if selection_revision != self.applied_selection_revision.get() {
            self.applied_selection_revision.set(selection_revision);
            let tracks = self.selected_tracks(&state.album_grid, library);
            self.tracks.set_rows(&tracks, state.album_grid.table.sort);
            self.tracks.sync_playing(playing_id);
            let visible = !tracks.is_empty();
            let changed = self.tracks_visible.replace(visible) != visible;
            if selection_changed || changed {
                self.apply_visibility();
            }
            changed
        } else {
            self.tracks.sync_playing(playing_id);
            false
        };

        if selected_index.is_none() && selection_changed {
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

    /// Applies the "Shuffle play" action for the currently selected album, as
    /// the toolbar's shuffle button would.
    pub fn shuffle(&self, state: &mut AppState, library: &dyn LibraryDataSource) -> Vec<Command> {
        let Some(key) = state.album_grid.selected_key().cloned() else {
            return Vec::new();
        };
        let cx = Ctx::with_library(&[], None, library);
        let mut out = Commands::new();
        state
            .album_grid
            .update(AlbumGridMsg::Shuffle(key), &cx, &mut out);
        out.into_vec()
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

    /// The command to toggle the star of `index`.
    pub fn toggle_star(&self, index: usize) -> Option<Command> {
        self.tracks.toggle_star(index)
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
        self.ui.set_visible(self.count.id(), active);
        self.ui.set_visible(self.sort_label.id(), active);
        self.ui.set_visible(self.sort.id(), active);
        self.ui.set_visible(self.size_label.id(), active);
        self.ui.set_visible(self.size.id(), active);
        self.ui.set_visible(self.shuffle.id(), active);
        self.ui
            .set_visible(self.close.id(), active && self.selected.get());
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

/// The tile size for a cover edge, reserving the caption strip below it.
fn tile_size(edge: f32) -> TileSize {
    TileSize::new(dip(edge), dip(edge + CAPTION_DIP)).gap(dip(GAP_DIP))
}

/// The sort combo's item labels, in [`AlbumSort::ALL`] order.
fn sort_labels() -> [&'static str; 4] {
    AlbumSort::ALL.map(|sort| sort.label())
}

/// The combo index of `sort`, or 0 when absent.
fn sort_index(sort: AlbumSort) -> usize {
    AlbumSort::ALL
        .iter()
        .position(|candidate| *candidate == sort)
        .unwrap_or(0)
}
