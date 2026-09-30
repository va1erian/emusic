//! The Genres view (#250), ported to the portable widget layer: the shared
//! name-sorted genre list, with track counts, in the reusable
//! [`NameCountsView`].
//!
//! All state and logic live in the shared
//! [`GenresView`](emusic_ui::views::genres::GenresView) model (`emusic-ui`);
//! this module only owns the count label and the list, and maps the list's
//! activation to the model's "Shuffle play" intent.

use std::cell::Cell;

use emusic_ui::library_api::{GenreInfo, LibraryDataSource};
use emusic_ui::search::SearchEngine;
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::genres::GenresMsg;
use emusic_ui::views::{Commands, Ctx};
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{HasText, Label};

use crate::app::Msg;
use crate::views::name_counts::{CountColumn, NameCountRow, NameCountsView};

/// Height of the "N genres" label, in design units.
const LABEL_HEIGHT: f32 = 20.0;
/// Width of the track-count column, in design units.
const COUNT_WIDTH: f32 = 72.0;
/// The count columns, in display order.
const COUNT_COLUMNS: [CountColumn; 1] = [CountColumn {
    title: "Tracks",
    width: COUNT_WIDTH,
}];

/// The Genres view: an "N genres" label over the virtual count list.
pub struct GenresView {
    ui: Ui<Msg>,
    label: Label<Msg>,
    rows: NameCountsView,
    /// The model revision the list rows were last built from.
    applied_revision: Cell<u64>,
}

impl GenresView {
    /// Creates the label and the (empty) virtual list.
    pub fn new(ui: &Ui<Msg>) -> GenresView {
        let label = Label::new(ui, Rect::default(), "0 genres").expect("create genres label");
        GenresView {
            ui: ui.clone(),
            label,
            rows: NameCountsView::new(ui, "Genre", &COUNT_COLUMNS),
            applied_revision: Cell::new(u64::MAX),
        }
    }

    /// Moves and sizes the whole view: the label on top, the list filling the
    /// rest.
    pub fn set_bounds(&self, bounds: Rect) {
        let header = dip(LABEL_HEIGHT).to_px(self.ui.dpi()).value();
        self.ui.apply_moves(&[(
            self.label.id(),
            Rect::new(bounds.left, bounds.top, bounds.right, bounds.top + header),
        )]);
        self.rows.set_bounds(Rect::new(
            bounds.left,
            bounds.top + header,
            bounds.right,
            bounds.bottom,
        ));
    }

    /// Shows or hides the whole view.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.label.id(), visible);
        self.rows.set_visible(visible);
    }

    /// Refreshes the shared model from `library` and rebuilds the list rows
    /// when the model changed.
    pub fn sync(
        &mut self,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
        search: &SearchEngine,
    ) {
        let cx = Ctx::with_library(&[], None, library).with_search(search);
        state.genres.refresh(&cx);
        if self.applied_revision.get() != state.genres.revision() {
            self.applied_revision.set(state.genres.revision());
            self.label.set_text(&state.genres.count_label());
            let rows = state.genres.rows().iter().map(row_for).collect();
            self.rows.set_rows(rows);
        }
    }

    /// Applies the "Shuffle play" action for the row's genre, returning the
    /// commands the model queued.
    pub fn shuffle(
        &self,
        row: usize,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
    ) -> Vec<Command> {
        let Some(name) = self.rows.name(row) else {
            return Vec::new();
        };
        let cx = Ctx::with_library(&[], None, library);
        let mut out = Commands::new();
        state.genres.update(GenresMsg::Shuffle(name), &cx, &mut out);
        out.into_vec()
    }
}

/// Builds one list row from a genre.
fn row_for(genre: &GenreInfo) -> NameCountRow {
    NameCountRow::new(genre.name.clone(), [genre.track_count.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_for_formats_the_genre_track_count() {
        let genre = GenreInfo {
            name: "Ambient".to_string(),
            track_count: 37,
        };
        let row = row_for(&genre);
        assert_eq!(row.name(), "Ambient");
        assert_eq!(row.count(0), "37");
    }
}
