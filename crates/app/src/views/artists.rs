//! Win32 Artists view (#248): the shared name-sorted artist list, with album
//! and track counts, in the reusable virtual [`NameCountsView`].
//!
//! All state and logic live in the shared
//! [`ArtistsView`](emusic_ui::views::artists::ArtistsView) model (`emusic-ui`);
//! this module only owns the count label and the native list, and maps the
//! list's "Shuffle play" context menu to the model.

use std::cell::Cell;

use emusic_ui::library_api::{ArtistInfo, LibraryDataSource};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::artists::ArtistsMsg;
use emusic_ui::views::{Commands, Ctx};
use xui::prelude::*;
use xui::{Control, Label, Layout, Menu, column, dip};

use crate::app::Msg;
use crate::views::name_counts::{CountColumn, NameCountRow, NameCountsView};

/// Height of the "N artists" label, in design units.
const LABEL_HEIGHT: f32 = 20.0;
/// Width of each count column, in design units.
const COUNT_WIDTH: f32 = 72.0;
/// The count columns, in display order.
const COUNT_COLUMNS: [CountColumn; 2] = [
    CountColumn {
        title: "Albums",
        width: COUNT_WIDTH,
    },
    CountColumn {
        title: "Tracks",
        width: COUNT_WIDTH,
    },
];

/// The Win32 Artists view: an "N artists" label over the virtual count list.
pub struct ArtistsView {
    label: Label,
    rows: NameCountsView,
    /// The model revision the list rows were last built from.
    applied_revision: Cell<u64>,
}

impl ArtistsView {
    /// Creates the label and the (empty) virtual list.
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        Ok(Self {
            label: Label::new(ui, Rect::default(), "0 artists")?,
            rows: NameCountsView::new(ui, "Artist", &COUNT_COLUMNS)?,
            applied_revision: Cell::new(u64::MAX),
        })
    }

    /// Refreshes the shared model from `library` and rebuilds the list rows
    /// when the model changed.
    pub fn sync(&mut self, state: &mut AppState, library: &dyn LibraryDataSource) {
        let cx = Ctx::with_library(&[], None, library);
        state.artists.refresh(&cx);
        if self.applied_revision.get() != state.artists.revision() {
            self.applied_revision.set(state.artists.revision());
            self.label.set_text(&state.artists.count_label());
            let rows = state.artists.rows().iter().map(row_for).collect();
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
        state
            .artists
            .update(ArtistsMsg::Shuffle(name), &cx, &mut out);
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

    /// The artist name of the row the context menu was opened on, if any.
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

    /// The "N artists" label above the virtual list.
    pub fn layout(&self) -> Layout {
        column![self.label.height(dip(LABEL_HEIGHT)), self.rows.fill(1)]
    }
}

impl AsControl for ArtistsView {
    fn control(&self) -> &Control {
        self.rows.control()
    }
}

/// Builds one list row from an artist.
fn row_for(artist: &ArtistInfo) -> NameCountRow {
    NameCountRow::new(
        artist.name.clone(),
        [
            artist.album_count.to_string(),
            artist.track_count.to_string(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_for_formats_the_artist_counts() {
        let artist = ArtistInfo {
            name: "Autechre".to_string(),
            album_count: 4,
            track_count: 51,
        };
        let row = row_for(&artist);
        assert_eq!(row.name(), "Autechre");
        assert_eq!(row.count(0), "4");
        assert_eq!(row.count(1), "51");
    }
}
