//! The selected album's track list (#113): a virtual [`ListView`] over the
//! shared [`TrackTable`] model, matching the egui frontend's table below the
//! grid.
//!
//! The display order, sorting and formatting all come from `emusic-ui` (the
//! same `TrackTable` and column helpers the Music view uses); this module only
//! owns the native list and turns its events into [`AlbumMsg`]s.

use std::cell::Cell;
use std::rc::Rc;

use emusic_ui::library_api::TrackInfo;
use emusic_ui::state::Command;
use emusic_ui::views::Ctx;
use emusic_ui::views::track_table::TrackTable;
use emusic_ui::views::track_table::columns::{self};
use win32ui::prelude::*;
use win32ui::{ColumnWidth, Fill, ListView, Menu, SortDirection, dip};

use crate::app::Msg;
use crate::views::album_grid::AlbumMsg;
use crate::views::track_table::{
    ContextAction, MusicModel, STAR_COLUMN, TrackRow, cell_text, column_index, play_column,
    run_context_action, star_column,
};

/// The selected album's tracks, in the shared table's display order.
pub(super) struct TrackList {
    list: ListView<TrackRow, Msg>,
    rows: Rc<Vec<TrackRow>>,
    /// The playing track id, shared with the `row_style` closure so the
    /// highlight follows playback without rebuilding the model.
    playing: Rc<Cell<Option<u64>>>,
    applied_revision: u64,
    context: Menu<Msg>,
    context_row: Cell<Option<usize>>,
    indicator: Cell<Option<(usize, bool)>>,
}

impl TrackList {
    /// Creates the (empty) virtual list with the shared track columns.
    pub(super) fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        let playing = Rc::new(Cell::new(None));
        let playing_for_style = Rc::clone(&playing);

        let mut list = ListView::new(ui)?
            .row_style(move |row: &TrackRow| {
                if Some(row.track.id) == playing_for_style.get() {
                    RowStyle::new().bold(true)
                } else {
                    RowStyle::default()
                }
            })
            .add_column(star_column())
            .add_column(play_column(Rc::clone(&playing)))
            .column("Title", Fill, |row: &TrackRow| row.text(0))
            .on_cell_click(|row, column, _point| {
                (column == STAR_COLUMN).then_some(Msg::Album(AlbumMsg::TableToggleStar(row)))
            })
            .on_activate(|row| Some(Msg::Album(AlbumMsg::TableActivate(row))))
            .on_sort(|column| Some(Msg::Album(AlbumMsg::TableSort(column))))
            .on_context(|row| Some(Msg::Album(AlbumMsg::TableContext(row))));
        for column in columns::COLUMNS {
            let id = column.id;
            list = list.column(
                column.label,
                ColumnWidth::Fixed(dip(column.initial_width)),
                move |row: &TrackRow| cell_text(row, id),
            );
        }

        let context = Menu::new()
            .item("Play", None, || {
                Msg::Album(AlbumMsg::TableAction(ContextAction::Play))
            })
            .item("Play next", None, || {
                Msg::Album(AlbumMsg::TableAction(ContextAction::PlayNext))
            })
            .item("Add to queue", None, || {
                Msg::Album(AlbumMsg::TableAction(ContextAction::AddToQueue))
            })
            .separator()
            .item("Star / Unstar", None, || {
                Msg::Album(AlbumMsg::TableAction(ContextAction::ToggleStar))
            });

        Ok(Self {
            list,
            rows: Rc::new(Vec::new()),
            playing,
            applied_revision: u64::MAX,
            context,
            context_row: Cell::new(None),
            indicator: Cell::new(None),
        })
    }

    /// Applies the current appearance metrics and zebra flag (#309).
    pub(super) fn apply_appearance(&self) {
        crate::appearance::apply_list(&self.list);
    }

    /// Rebuilds the rows from the shared table when its display order changed,
    /// and refreshes the playing highlight and sort arrow.
    pub(super) fn sync(
        &mut self,
        table: &TrackTable,
        tracks: &[&TrackInfo],
        playing_id: Option<u64>,
    ) {
        if table.revision() != self.applied_revision {
            let cx = Ctx::new(tracks, playing_id);
            let rows: Vec<TrackRow> = (0..table.len())
                .filter_map(|index| table.row(index, &cx).map(|view| TrackRow::new(view.track)))
                .collect();
            self.rows = Rc::new(rows);
            self.list.set_model(MusicModel {
                rows: Rc::clone(&self.rows),
            });
            self.applied_revision = table.revision();
        }

        if self.playing.get() != playing_id {
            self.playing.set(playing_id);
            let len = self.rows.len();
            self.list.rows_changed(0..len);
        }

        self.show_sort_indicator(table.sort);
    }

    /// The command to play `row` in the context of the whole album.
    pub(super) fn activate(&self, row: usize) -> Option<Command> {
        let track = self.rows.as_slice().get(row)?;
        let context: Vec<u64> = self.rows.iter().map(|row| row.track.id).collect();
        Some(Command::play_track(track.track.id, context))
    }

    /// Flips the star for `row`, repaints that cell and returns the command to
    /// persist it. The click never moved the selection.
    pub(super) fn toggle_star(&self, row: usize) -> Option<Command> {
        let entry = self.rows.as_slice().get(row)?;
        let id = entry.flip_star();
        self.list.rows_changed(row..row + 1);
        Some(Command::ToggleStarred(id))
    }

    /// Remembers the row that opened the context menu.
    pub(super) fn set_context_row(&self, row: usize) {
        self.context_row.set(Some(row));
    }

    /// The track context menu, shown by the app with [`Ui::popup`].
    pub(super) fn context_menu(&self) -> &Menu<Msg> {
        &self.context
    }

    /// Runs a context action on the row that opened the menu.
    pub(super) fn run_context(
        &self,
        action: ContextAction,
        hwnd: win32ui::Hwnd,
    ) -> Option<Command> {
        let row = self
            .context_row
            .get()
            .and_then(|index| self.rows.as_slice().get(index))?;
        run_context_action(action, &row.track, hwnd)
    }

    /// Shows the sort arrow on the header for the table's current sort.
    fn show_sort_indicator(&self, sort: emusic_ui::views::track_table::sort::SortState) {
        let wanted = sort
            .key
            .and_then(|id| column_index(id).map(|index| (index, sort.ascending)));
        if self.indicator.get() == wanted {
            return;
        }
        if let Some((previous, _)) = self.indicator.get() {
            self.list.clear_sort_indicator(previous);
        }
        if let Some((index, ascending)) = wanted {
            let direction = if ascending {
                SortDirection::Ascending
            } else {
                SortDirection::Descending
            };
            self.list.set_sort_indicator(index, direction);
        }
        self.indicator.set(wanted);
    }
}

impl AsControl for TrackList {
    fn control(&self) -> &Control {
        self.list.control()
    }
}
