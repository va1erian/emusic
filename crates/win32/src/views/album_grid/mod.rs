//! Win32 Albums view (#113): a custom-painted, virtualized [`GridView`] of
//! cover tiles, with the selected album's tracks in a virtual [`ListView`]
//! below.
//!
//! All state and logic live in the shared [`AlbumGrid`] model (`emusic-ui`):
//! sorting, identity, the album list and the selected album's track table.
//! This module only owns the native controls, draws the tiles (through
//! [`tile::content`]) and turns control events into [`AlbumMsg`]s.
//!
//! [`ListView`]: win32ui::ListView

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use emusic_ui::library_api::{LibraryDataSource, TrackInfo};
use emusic_ui::state::AppState;
use emusic_ui::views::album_grid::models::{AlbumKey, AlbumSort};
use emusic_ui::views::album_grid::{
    self, AlbumGrid, AlbumGridMsg, DEFAULT_TILE_SIZE, MAX_TILE_SIZE, MIN_TILE_SIZE,
};
use emusic_ui::views::track_table::TrackTableMsg;
use emusic_ui::views::{Commands, Ctx};
use emusic_ui::waker::WakerHandle;
use win32ui::prelude::*;
use win32ui::{Button, ComboBox, Label, column, dip, row};

use crate::app::Msg;
use crate::views::music::{ContextAction, column_id};

mod thumbs;
mod tile;
mod tracks;

use self::thumbs::ThumbState;
use self::tile::{AlbumTile, TileModel};
use self::tracks::TrackList;

/// The toolbar band height, in design units.
const TOOLBAR_HEIGHT: f32 = 34.0;
/// Nominal control heights, used to centre each toolbar control vertically
/// within the band.
const LABEL_HEIGHT: f32 = 18.0;
const COMBO_HEIGHT: f32 = 26.0;
const SLIDER_HEIGHT: f32 = 28.0;
const BUTTON_HEIGHT: f32 = 28.0;

/// Wraps a toolbar control so it is centred vertically in the [`TOOLBAR_HEIGHT`]
/// band: symmetric top/bottom margins shrink its area to `height`, and the
/// control fills that area.
fn centered(control: &impl AsControl, height: f32) -> Layout {
    let pad = ((TOOLBAR_HEIGHT - height) / 2.0).max(0.0);
    Layout::row()
        .item(control.fill(1))
        .margins(Insets::new(dip(0.0), dip(pad), dip(0.0), dip(pad)))
}

/// Everything the Albums view's controls can ask the app to do.
pub enum AlbumMsg {
    /// A tile was clicked; select that album.
    Select(usize),
    /// A tile was double-clicked; play the album.
    Activate(usize),
    /// The sort combo changed.
    SetSort(AlbumSort),
    /// The tile-size slider moved (live).
    SetTileSize(f32),
    /// The tile-size slider was released; rebuild the cover cache.
    CommitTileSize(f32),
    /// The "Close album" button was clicked.
    CloseAlbum,
    /// The "Shuffle" button was clicked; shuffle the selected album.
    Shuffle,
    /// A track column header was clicked.
    TableSort(usize),
    /// A track row was double-clicked; play it.
    TableActivate(usize),
    /// A track row was right-clicked; open its context menu.
    TableContext(usize),
    /// A track context-menu entry was chosen.
    TableAction(ContextAction),
}

/// The Win32 Albums view: the sort/size toolbar, the album grid and the
/// selected album's track list.
pub struct AlbumGridView {
    count: Label,
    sort_label: Label,
    sort: ComboBox<AlbumSort, Msg>,
    size_label: Label,
    size: Slider<Msg>,
    shuffle: Button<Msg>,
    close: Button<Msg>,
    grid: GridView<AlbumTile, Msg>,
    tracks: TrackList,
    /// The shared thumbnail cache and the sink its uploads go through.
    thumbs: Rc<RefCell<ThumbState>>,
    /// The theme the tile painter draws text/placeholder colours from.
    theme: Rc<Cell<Theme>>,
    waker: WakerHandle,
    dpi: u32,
    /// The albums currently in the grid, for index-to-key mapping.
    tiles: Rc<Vec<AlbumTile>>,
    /// The album-grid revision the model was last built from.
    grid_revision: u64,
    /// The model tile size last applied to the grid.
    applied_tile_size: f32,
    /// The grid viewport width last resynced, so a window resize recomputes the
    /// scrollable extent (the number of columns changed).
    applied_width: i32,
    /// Whether the Albums view is the active central view.
    active: Cell<bool>,
    /// Whether an album is selected (drives the close button and shuffle).
    selected: Cell<bool>,
    /// Whether the track list is currently shown.
    tracks_visible: Cell<bool>,
}

impl AlbumGridView {
    /// Creates the view and its (empty) controls. `waker` lets the thumbnail
    /// workers repaint the grid when a decode finishes.
    pub fn new(ui: &mut Ui<Msg>, waker: WakerHandle) -> Result<Self> {
        let dpi = ui.dpi();
        let caption_px = dip(tile::CAPTION_DIP).to_px(dpi).value();
        let cover_px = dip(DEFAULT_TILE_SIZE).to_px(dpi).value();
        let thumbs = Rc::new(RefCell::new(ThumbState::new(cover_px, waker.clone())));
        let theme = Rc::new(Cell::new(ui.theme()));

        let grid = GridView::<AlbumTile, Msg>::new(ui)?
            .tile_size(
                dip(MIN_TILE_SIZE + tile::CAPTION_DIP)..dip(MAX_TILE_SIZE + tile::CAPTION_DIP),
            )
            .content(tile::content(
                Rc::clone(&thumbs),
                Rc::clone(&theme),
                dpi,
                caption_px,
            ))
            .on_select(|index| Some(Msg::Album(AlbumMsg::Select(index))))
            .on_activate(|index| Some(Msg::Album(AlbumMsg::Activate(index))));
        grid.set_tile_size(dip(DEFAULT_TILE_SIZE + tile::CAPTION_DIP));

        let sort = ComboBox::new(ui, AlbumSort::ALL.map(|sort| (sort.label(), sort)))?
            .select(&AlbumSort::default())
            .on_select(|sort| Some(Msg::Album(AlbumMsg::SetSort(*sort))));

        let size = Slider::new(ui, f64::from(MIN_TILE_SIZE)..=f64::from(MAX_TILE_SIZE))?
            .value(f64::from(DEFAULT_TILE_SIZE))
            .on_change(|value| Some(Msg::Album(AlbumMsg::SetTileSize(value as f32))))
            .on_commit(|value| Some(Msg::Album(AlbumMsg::CommitTileSize(value as f32))));

        let shuffle = Button::new(ui, "Shuffle")?.on_click(|| Some(Msg::Album(AlbumMsg::Shuffle)));
        let close =
            Button::new(ui, "Close album")?.on_click(|| Some(Msg::Album(AlbumMsg::CloseAlbum)));
        let tracks = TrackList::new(ui)?;

        let view = Self {
            count: Label::new(ui, Rect::default(), "0 albums")?,
            sort_label: Label::new(ui, Rect::default(), "Sort")?,
            sort,
            size_label: Label::new(ui, Rect::default(), "Size")?,
            size,
            shuffle,
            close,
            grid,
            tracks,
            thumbs,
            theme,
            waker,
            dpi,
            tiles: Rc::new(Vec::new()),
            grid_revision: u64::MAX,
            applied_tile_size: DEFAULT_TILE_SIZE,
            applied_width: 0,
            active: Cell::new(true),
            selected: Cell::new(false),
            tracks_visible: Cell::new(false),
        };
        view.apply_visibility();
        Ok(view)
    }

    /// Shows or hides the whole view (central-area routing). The track list
    /// and close button stay governed by the selection.
    pub fn set_visible(&self, visible: bool) {
        if self.active.get() != visible {
            self.active.set(visible);
            self.apply_visibility();
        }
    }

    /// Pushes each control's visibility from the active/selection state, so a
    /// hidden view never shows a stray control.
    fn apply_visibility(&self) {
        let active = self.active.get();
        self.count.set_visible(active);
        self.sort_label.set_visible(active);
        self.sort.set_visible(active);
        self.size_label.set_visible(active);
        self.size.set_visible(active);
        self.shuffle.set_visible(active);
        self.close.set_visible(active && self.selected.get());
        self.grid.set_visible(active);
        self.tracks.set_visible(active && self.tracks_visible.get());
    }

    /// The view's layout: a sort/size toolbar over the grid and the selected
    /// album's track list. The track list is hidden (and takes no space) until
    /// an album is selected.
    pub fn layout(&self) -> Layout {
        column![
            row![
                centered(&self.count, LABEL_HEIGHT).width(dip(96.0)),
                centered(&self.sort_label, LABEL_HEIGHT).width(dip(34.0)),
                centered(&self.sort, COMBO_HEIGHT).width(dip(150.0)),
                centered(&self.size_label, LABEL_HEIGHT).width(dip(34.0)),
                centered(&self.size, SLIDER_HEIGHT).fill(1),
                centered(&self.shuffle, BUTTON_HEIGHT).width(dip(88.0)),
                centered(&self.close, BUTTON_HEIGHT).width(dip(104.0)),
            ]
            .spacing(dip(6.0))
            .height(dip(TOOLBAR_HEIGHT)),
            self.grid.fill(1),
            self.tracks.fill(1),
        ]
        .spacing(dip(4.0))
    }

    /// Pushes the shared model into the controls. Returns whether the layout
    /// must be recomputed (the track list was shown or hidden).
    pub fn sync(
        &mut self,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        playing_id: Option<u64>,
        theme: Theme,
    ) -> bool {
        self.theme.set(theme);
        self.thumbs.borrow_mut().drain();

        let grid_revision = {
            let cx = Ctx::with_library(&[], playing_id, library);
            state.album_grid.refresh(&cx);
            state.album_grid.revision()
        };

        self.count
            .set_text(&format!("{} albums", state.album_grid.len()));
        if self.sort.selected() != Some(&state.album_grid.sort) {
            self.sort.set_selected(&state.album_grid.sort);
        }
        let tile_size = f64::from(state.album_grid.tile_size);
        if (self.size.current_value() - tile_size).abs() > f64::EPSILON {
            self.size.set_value(tile_size);
        }
        let selected = state.album_grid.selected_key().is_some();
        self.shuffle.set_enabled(selected);
        let selection_changed = self.selected.replace(selected) != selected;

        if grid_revision != self.grid_revision {
            self.rebuild_grid(&state.album_grid, library);
            self.grid_revision = grid_revision;
        }
        if (self.applied_tile_size - state.album_grid.tile_size).abs() > f32::EPSILON {
            self.grid
                .set_tile_size(dip(state.album_grid.tile_size + tile::CAPTION_DIP));
            self.applied_tile_size = state.album_grid.tile_size;
        }
        // The number of columns (and so the scrollable extent) depends on the
        // viewport width; the `GridView` does not observe its own resize, so
        // resync it here when the layout gave it a new width.
        let width = self.grid.bounds().width();
        if width != self.applied_width {
            self.applied_width = width;
            self.grid
                .set_tile_size(dip(state.album_grid.tile_size + tile::CAPTION_DIP));
        }

        let selected_index = state
            .album_grid
            .selected_key()
            .and_then(|key| self.tiles.iter().position(|tile| &tile.key == key));
        if self.grid.selected() != selected_index {
            self.grid.set_selected(selected_index);
        }

        let selected_tracks = self.selected_tracks(&state.album_grid, library);
        {
            let cx = Ctx::new(&selected_tracks, playing_id);
            state.album_grid.table.refresh(&cx);
        }
        self.tracks
            .sync(&state.album_grid.table, &selected_tracks, playing_id);

        let visible = !selected_tracks.is_empty();
        let tracks_changed = self.tracks_visible.replace(visible) != visible;
        if selection_changed || tracks_changed {
            self.apply_visibility();
        }
        selection_changed || tracks_changed
    }

    /// Applies one control event, queueing any resulting commands. `ui` is
    /// needed to open the track list's context menu at the cursor.
    pub fn update(
        &mut self,
        msg: AlbumMsg,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        playing_id: Option<u64>,
        ui: &mut Ui<Msg>,
        out: &mut Commands,
    ) {
        match msg {
            AlbumMsg::Select(index) => {
                if let Some(key) = self.key_at(index) {
                    let cx = Ctx::with_library(&[], playing_id, library);
                    state
                        .album_grid
                        .update(AlbumGridMsg::TileClicked(key), &cx, out);
                }
            }
            AlbumMsg::Activate(index) => {
                if let Some(key) = self.key_at(index) {
                    let cx = Ctx::with_library(&[], playing_id, library);
                    state
                        .album_grid
                        .update(AlbumGridMsg::TileActivated(key), &cx, out);
                }
            }
            AlbumMsg::SetSort(sort) => {
                let cx = Ctx::with_library(&[], playing_id, library);
                state
                    .album_grid
                    .update(AlbumGridMsg::SetSort(sort), &cx, out);
            }
            AlbumMsg::SetTileSize(size) => {
                let cx = Ctx::with_library(&[], playing_id, library);
                state
                    .album_grid
                    .update(AlbumGridMsg::SetTileSize(size), &cx, out);
            }
            AlbumMsg::CommitTileSize(size) => {
                let cover_px = dip(size).to_px(self.dpi).value();
                *self.thumbs.borrow_mut() = ThumbState::new(cover_px, self.waker.clone());
                self.grid.set_tile_size(dip(size + tile::CAPTION_DIP));
            }
            AlbumMsg::CloseAlbum => {
                let cx = Ctx::with_library(&[], playing_id, library);
                state.album_grid.update(AlbumGridMsg::CloseAlbum, &cx, out);
            }
            AlbumMsg::Shuffle => {
                if let Some(key) = state.album_grid.selected_key().cloned() {
                    let cx = Ctx::with_library(&[], playing_id, library);
                    state
                        .album_grid
                        .update(AlbumGridMsg::Shuffle(key), &cx, out);
                }
            }
            AlbumMsg::TableSort(column) => {
                let Some(id) = column_id(column) else {
                    return;
                };
                let selected = self.selected_tracks(&state.album_grid, library);
                let cx = Ctx::new(&selected, playing_id);
                state.album_grid.update(
                    AlbumGridMsg::Table(TrackTableMsg::HeaderClicked(id)),
                    &cx,
                    out,
                );
            }
            AlbumMsg::TableActivate(row) => {
                if let Some(command) = self.tracks.activate(row) {
                    out.push(command);
                }
            }
            AlbumMsg::TableContext(row) => {
                self.tracks.set_context_row(row);
                ui.popup(self.tracks.context_menu(), ui.cursor_position());
            }
            AlbumMsg::TableAction(action) => {
                if let Some(command) = self.tracks.run_context(action, ui.hwnd()) {
                    out.push(command);
                }
            }
        }
    }

    /// Rebuilds the grid model from the album list, resolving each album's
    /// cover source from the shared catalog.
    fn rebuild_grid(&mut self, grid: &AlbumGrid, library: &dyn LibraryDataSource) {
        let meta = album_grid::album_meta(library);
        let tiles: Vec<AlbumTile> = grid
            .albums()
            .iter()
            .map(|album| {
                let key = AlbumKey::of(album);
                let art_path = meta
                    .get(&key)
                    .map(|meta| meta.art_path.clone())
                    .unwrap_or_default();
                AlbumTile {
                    key,
                    name: album.name.clone(),
                    artist: album.artist.clone(),
                    year: album.year,
                    art_path,
                }
            })
            .collect();
        self.tiles = Rc::new(tiles);
        self.grid.set_model(TileModel {
            tiles: Rc::clone(&self.tiles),
        });
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
        self.tiles
            .as_slice()
            .get(index)
            .map(|tile| tile.key.clone())
    }
}
