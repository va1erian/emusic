//! The Starred view (#251), ported to the portable widget layer: an
//! "N starred" header above the shared track table listing the starred tracks.
//!
//! The count and the table's sort/selection live in `emusic-ui`'s
//! [`StarredView`](emusic_ui::views::starred::StarredView); this module only
//! owns the controls and hands the starred tracks to the reusable
//! [`TrackView`]. The model is refreshed every sync, so starring a track from
//! the Music table or this view's own context menu updates the list on the
//! next tick.

use std::cell::Cell;

use emusic_ui::library_api::{LibraryDataSource, TrackInfo};
use emusic_ui::search::SearchEngine;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::Ctx;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{HasText, Label};

use crate::app::Msg;
use crate::make::make;
use crate::views::track_table::TrackView;
use xui::xui_core::arrange::label;

/// The header band height, in design units.
const HEADER_HEIGHT: f32 = 26.0;

/// The Starred view: the count header and the starred track table.
pub struct StarredView {
    ui: Ui<Msg>,
    header: Label<Msg>,
    table: TrackView,
    /// The model revision the controls were last built from.
    applied_revision: Cell<u64>,
}

impl StarredView {
    /// Creates the header and the (empty) track table.
    pub fn new(ui: &Ui<Msg>) -> StarredView {
        let header = make(ui, label("0 starred")).expect("create starred header");
        StarredView {
            ui: ui.clone(),
            header,
            table: TrackView::new(ui),
            applied_revision: Cell::new(u64::MAX),
        }
    }

    /// Moves and sizes the whole view: the header on top, the table filling the
    /// rest.
    pub fn set_bounds(&self, bounds: Rect) {
        let header = dip(HEADER_HEIGHT).to_px(self.ui.dpi()).value();
        self.ui.apply_moves(&[(
            self.header.id(),
            Rect::new(bounds.left, bounds.top, bounds.right, bounds.top + header),
        )]);
        self.table.set_bounds(Rect::new(
            bounds.left,
            bounds.top + header,
            bounds.right,
            bounds.bottom,
        ));
    }

    /// Shows or hides the whole view (its header and table).
    /// The track table's client origin, to anchor the row context menu.
    pub fn context_origin(&self) -> xui::xui_core::geometry::Point {
        self.table.context_origin()
    }

    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.header.id(), visible);
        self.table.set_visible(visible);
    }

    /// Refreshes the shared model from the library and mirrors it into the
    /// controls when the starred set (or its sort) changed.
    pub fn sync(
        &mut self,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        search: &SearchEngine,
        playing_id: Option<u64>,
    ) {
        let tracks = library.starred_tracks();
        state
            .starred
            .refresh(&Ctx::new(&tracks, playing_id).with_search(search));
        if state.starred.revision() != self.applied_revision.get() {
            self.applied_revision.set(state.starred.revision());
            self.header.set_text(&state.starred.count_label());
            let visible = state.starred.visible_tracks(&tracks);
            self.table.set_rows(&visible, state.starred.table.sort);
        }
        self.table.sync_playing(playing_id);
    }

    /// Rebuilds the table after a header click, preserving the new sort and the
    /// active search filter.
    pub fn resort(&mut self, state: &AppState, library: &dyn LibraryDataSource) {
        let tracks = library.starred_tracks();
        let visible = state.starred.visible_tracks(&tracks);
        self.table.set_rows(&visible, state.starred.table.sort);
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<Command> {
        self.table.activate(index)
    }

    /// The command to toggle the star of `index`.
    pub fn toggle_star(&self, index: usize) -> Option<Command> {
        self.table.toggle_star(index)
    }

    /// The track at `index`, for a context action.
    pub fn track(&self, index: usize) -> Option<TrackInfo> {
        self.table.track(index).cloned()
    }
}
