//! The Artists view (#248), ported to the portable widget layer: the shared
//! name-sorted artist list, with album and track counts, in the reusable
//! [`NameCountsView`].
//!
//! All state and logic live in the shared
//! [`ArtistsView`](emusic_ui::views::artists::ArtistsView) model (`emusic-ui`);
//! this module only owns the count label and the list, and maps the list's
//! activation to the model's "Shuffle play" intent.

use std::cell::Cell;

use emusic_ui::library_api::{ArtistInfo, LibraryDataSource};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::artists::ArtistsMsg;
use emusic_ui::views::{Commands, Ctx};
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{HasText, Label};

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

/// The Artists view: an "N artists" label over the virtual count list.
pub struct ArtistsView {
    ui: Ui<Msg>,
    label: Label<Msg>,
    rows: NameCountsView,
    /// The model revision the list rows were last built from.
    applied_revision: Cell<u64>,
}

impl ArtistsView {
    /// Creates the label and the (empty) virtual list.
    pub fn new(ui: &Ui<Msg>) -> ArtistsView {
        let label = Label::new(ui, Rect::default(), "0 artists").expect("create artists label");
        ArtistsView {
            ui: ui.clone(),
            label,
            rows: NameCountsView::new(ui, "Artist", &COUNT_COLUMNS),
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

    /// Applies the "Shuffle play" action for the row's artist, returning the
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
        state
            .artists
            .update(ArtistsMsg::Shuffle(name), &cx, &mut out);
        out.into_vec()
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
