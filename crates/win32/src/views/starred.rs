//! Win32 Starred view (#251): an "N starred" header above the shared track
//! table listing the starred tracks.
//!
//! The count and the table's sort/selection live in `emusic-ui`'s
//! [`StarredView`](emusic_ui::views::starred::StarredView); this module only
//! owns the native controls and hands the starred tracks to the reusable
//! [`TrackView`]. The model is refreshed every sync, so starring a track from
//! the Music table, the now-playing panel or this view's own context menu
//! updates the list on the next tick.

use std::cell::Cell;

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::state::AppState;
use emusic_ui::views::Ctx;
use win32ui::prelude::*;
use win32ui::{Control, Label, Menu, column, dip};

use crate::app::Msg;
use crate::views::track_table::TrackView;

/// The header band height, in design units.
const HEADER_HEIGHT: f32 = 26.0;

/// The Win32 Starred view: the count header and the starred track table.
pub struct StarredView {
    header: Label,
    table: TrackView,
    /// The model revision the controls were last built from.
    applied_revision: Cell<u64>,
}

impl StarredView {
    /// Creates the header and the (empty) track table.
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        Ok(Self {
            header: Label::new(ui, Rect::default(), "0 starred")?,
            table: TrackView::new(ui)?,
            applied_revision: Cell::new(u64::MAX),
        })
    }

    /// Refreshes the shared model from the library and mirrors it into the
    /// controls when the starred set (or its sort) changed.
    pub fn sync(
        &mut self,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        playing_id: Option<u64>,
    ) {
        let tracks = library.starred_tracks();
        state.starred.refresh(&Ctx::new(&tracks, playing_id));
        if state.starred.revision() != self.applied_revision.get() {
            self.applied_revision.set(state.starred.revision());
            self.header.set_text(&state.starred.count_label());
            self.table.set_rows(&tracks, state.starred.table.sort);
        }
        self.table.sync_playing(playing_id);
    }

    /// Rebuilds the table after a header click, preserving the new sort.
    pub fn resort(&mut self, state: &AppState, library: &dyn LibraryDataSource) {
        let tracks = library.starred_tracks();
        self.table.set_rows(&tracks, state.starred.table.sort);
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<emusic_ui::state::Command> {
        self.table.activate(index)
    }

    /// Runs a context action on the row that opened the menu.
    pub fn run_context(
        &self,
        action: crate::views::track_table::ContextAction,
        hwnd: win32ui::Hwnd,
    ) -> Option<emusic_ui::state::Command> {
        self.table.run_context(action, hwnd)
    }

    pub fn set_context_row(&self, row: usize) {
        self.table.set_context_row(row);
    }

    pub fn context_menu(&self) -> &Menu<Msg> {
        self.table.context_menu()
    }

    /// Shows or hides the whole view (its header and table).
    pub fn set_visible(&self, visible: bool) {
        self.header.set_visible(visible);
        self.table.set_visible(visible);
    }

    /// The count header above the track table.
    pub fn layout(&self) -> Layout {
        column![self.header.height(dip(HEADER_HEIGHT)), self.table.fill(1)]
    }
}

impl AsControl for StarredView {
    fn control(&self) -> &Control {
        self.table.control()
    }
}
