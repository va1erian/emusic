//! Win32 Playlist view (#476): a header with the playlist's name, entry count
//! and Play / Shuffle / Export actions above the shared track table, kept in
//! the playlist's own order.
//!
//! The entries and the table's sort/selection live in `emusic-ui`'s
//! [`PlaylistView`](emusic_ui::views::playlist::PlaylistView); this module only
//! owns the native controls and hands the resolved tracks to the reusable
//! [`TrackView`]. The table is also a reorder drop target while no sort column
//! is active.

use std::cell::Cell;

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::playlist as model;
use emusic_ui::views::{Commands, Ctx};
use win32ui::prelude::*;
use win32ui::{Button, Control, Label, Menu, Rect, column, dip, row};

use crate::app::Msg;
use crate::views::track_menu::track_context_menu;
use crate::views::track_table::{TableMode, TrackView};

/// Header row height, in device-independent pixels.
const HEADER_HEIGHT: f32 = 30.0;
/// Width of the header's action buttons, in device-independent pixels.
const BUTTON_WIDTH: f32 = 92.0;

/// The Win32 Playlist view: the header and the ordered track table.
pub struct PlaylistView {
    name: Label,
    count: Label,
    play: Button<Msg>,
    shuffle: Button<Msg>,
    export: Button<Msg>,
    table: TrackView,
    /// The model revision the controls were last built from.
    applied_revision: Cell<u64>,
}

impl PlaylistView {
    /// Creates the header controls and the (empty) reorderable table.
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        Ok(Self {
            name: Label::new(ui, Rect::default(), "")?,
            count: Label::new(ui, Rect::default(), "0 tracks")?,
            play: Button::new(ui, "Play")?.on_click(|| Some(Msg::PlaylistPlay(false))),
            shuffle: Button::new(ui, "Shuffle")?.on_click(|| Some(Msg::PlaylistPlay(true))),
            export: Button::new(ui, "Export…")?.on_click(|| Some(Msg::PlaylistExport)),
            table: TrackView::new(ui, TableMode::Reorder)?,
            applied_revision: Cell::new(u64::MAX),
        })
    }

    /// Refreshes the shared model from the library and mirrors it into the
    /// controls when the entries (or their sort) changed.
    pub fn sync(
        &mut self,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        playing_id: Option<u64>,
        id: u64,
    ) {
        let resolved = model::resolve(library, id);
        let cx = Ctx::new(&resolved.tracks, playing_id);
        state.playlist.refresh(&resolved.items, &cx);

        self.name.set_text(&playlist_name(library, id));
        if state.playlist.revision() != self.applied_revision.get() {
            self.applied_revision.set(state.playlist.revision());
            self.count.set_text(&state.playlist.count_label());
            self.table
                .set_rows(&resolved.tracks, state.playlist.table.sort);
        }
        self.table.set_reorder_ok(state.playlist.can_reorder());
        self.table.sync_playing(playing_id);
    }

    /// Rebuilds the table after a header click, preserving the new sort.
    pub fn resort(&mut self, state: &AppState, library: &dyn LibraryDataSource, id: u64) {
        let resolved = model::resolve(library, id);
        self.table
            .set_rows(&resolved.tracks, state.playlist.table.sort);
    }

    /// Forgets the recorded reorder drag after another target took the drop.
    pub fn end_drag(&self) {
        self.table.end_drag();
    }

    /// The rows the reorder drag carrying `payload` started from; empty for
    /// a drag that did not start in this table.
    pub fn dragged_rows(&self, payload: Option<&[u8]>) -> Vec<usize> {
        self.table.dragged_rows(payload)
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<Command> {
        self.table.activate(index)
    }

    /// The command to toggle the star of `index` (a star-cell click).
    pub fn toggle_star(&self, index: usize) -> Option<Command> {
        self.table.toggle_star(index)
    }

    /// Runs a context action on the row that opened the menu.
    pub fn run_context(
        &self,
        action: crate::views::track_table::ContextAction,
        hwnd: win32ui::Hwnd,
    ) -> Option<Command> {
        self.table.run_context(action, hwnd)
    }

    pub fn set_context_row(&self, row: usize) {
        self.table.set_context_row(row);
    }

    /// The track whose context menu is open, if any.
    pub fn context_track(&self) -> Option<emusic_ui::library_api::TrackInfo> {
        self.table.context_track()
    }

    /// The table's context menu, with the playlist's own "Remove from playlist".
    pub fn context_menu(&self, playlists: &[emusic_ui::library_api::PlaylistInfo]) -> Menu<Msg> {
        track_context_menu(playlists, true)
    }

    /// Starts dragging the table's `rows` (#476).
    pub fn begin_drag(&self, rows: &[usize]) {
        self.table.begin_drag(rows);
    }

    /// Queues adding the context row's selection to playlist `id` (#476).
    pub fn add_to_playlist(&self, id: u64) -> Option<Command> {
        self.table.add_to_playlist(id)
    }

    /// Queues removing the selected entries from playlist `id`.
    pub fn remove_selected(&self, state: &AppState, id: u64) -> Option<Command> {
        let mut out = Commands::new();
        state.playlist.remove_selected(id, &mut out);
        out.into_vec().into_iter().next()
    }

    /// Shows or hides the whole view (its header and table).
    pub fn set_visible(&self, visible: bool) {
        self.name.set_visible(visible);
        self.count.set_visible(visible);
        self.play.set_visible(visible);
        self.shuffle.set_visible(visible);
        self.export.set_visible(visible);
        self.table.set_visible(visible);
    }

    /// Applies the current appearance metrics and zebra flag.
    pub fn apply_appearance(&self) {
        self.table.apply_appearance();
    }

    /// The header row and the reorderable track table.
    pub fn layout(&self) -> Layout {
        let header = row![
            self.name.fill(1),
            self.count.width(dip(90.0)),
            self.play.width(dip(BUTTON_WIDTH)),
            self.shuffle.width(dip(BUTTON_WIDTH)),
            self.export.width(dip(BUTTON_WIDTH)),
        ]
        .spacing(dip(8.0))
        .height(dip(HEADER_HEIGHT));
        column![header, self.table.fill(1)]
    }
}

impl AsControl for PlaylistView {
    fn control(&self) -> &Control {
        self.table.control()
    }
}

/// The playlist's current name, or an empty label when it vanished.
fn playlist_name(library: &dyn LibraryDataSource, id: u64) -> String {
    library
        .playlists()
        .iter()
        .find(|playlist| playlist.id == id)
        .map(|playlist| playlist.name.clone())
        .unwrap_or_default()
}
