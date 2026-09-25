//! Win32 Genres view (#250): the shared name-sorted genre list, with track
//! counts, in the reusable virtual [`NameCountsView`].
//!
//! All state and logic live in the shared
//! [`GenresView`](emusic_ui::views::genres::GenresView) model (`emusic-ui`);
//! this module only owns the count label and the native list, and maps the
//! list's "Shuffle play" context menu to the model.

use std::cell::Cell;

use emusic_ui::library_api::{GenreInfo, LibraryDataSource};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::genres::GenresMsg;
use emusic_ui::views::{Commands, Ctx};
use win32ui::prelude::*;
use win32ui::{Control, Label, Layout, Menu, column, dip};

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

/// The Win32 Genres view: an "N genres" label over the virtual count list.
pub struct GenresView {
    label: Label,
    rows: NameCountsView,
    /// The model revision the list rows were last built from.
    applied_revision: Cell<u64>,
}

impl GenresView {
    /// Creates the label and the (empty) virtual list.
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        Ok(Self {
            label: Label::new(ui, Rect::default(), "0 genres")?,
            rows: NameCountsView::new(ui, "Genre", &COUNT_COLUMNS)?,
            applied_revision: Cell::new(u64::MAX),
        })
    }

    /// Refreshes the shared model from `library` and rebuilds the list rows
    /// when the model changed.
    pub fn sync(&mut self, state: &mut AppState, library: &dyn LibraryDataSource) {
        let cx = Ctx::with_library(&[], None, library);
        state.genres.refresh(&cx);
        if self.applied_revision.get() != state.genres.revision() {
            self.applied_revision.set(state.genres.revision());
            self.label.set_text(&state.genres.count_label());
            let rows = state.genres.rows().iter().map(row_for).collect();
            self.rows.set_rows(rows);
        }
    }

    /// Applies the "Shuffle play" action for `name`, returning the commands the
    /// model queued.
    pub fn shuffle(
        &self,
        name: String,
        state: &mut AppState,
        library: &dyn LibraryDataSource,
    ) -> Vec<Command> {
        let cx = Ctx::with_library(&[], None, library);
        let mut out = Commands::new();
        state.genres.update(GenresMsg::Shuffle(name), &cx, &mut out);
        out.into_vec()
    }

    /// Remembers the row the context menu was opened on.
    pub fn set_context_row(&self, row: usize) {
        self.rows.set_context_row(row);
    }

    /// The list's row context menu.
    pub fn context_menu(&self) -> &Menu<Msg> {
        self.rows.context_menu()
    }

    /// The genre name of the row the context menu was opened on, if any.
    pub fn context_name(&self) -> Option<String> {
        self.rows.context_name()
    }

    /// Shows or hides the whole view.
    pub fn set_visible(&self, visible: bool) {
        self.label.set_visible(visible);
        self.rows.set_visible(visible);
    }

    /// Applies the current appearance metrics and zebra flag (#309).
    pub fn apply_appearance(&self) {
        self.rows.apply_appearance();
    }

    /// The "N genres" label above the virtual list.
    pub fn layout(&self) -> Layout {
        column![self.label.height(dip(LABEL_HEIGHT)), self.rows.fill(1)]
    }
}

impl AsControl for GenresView {
    fn control(&self) -> &Control {
        self.rows.control()
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
