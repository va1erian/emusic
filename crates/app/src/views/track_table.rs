//! The shared track table (#107, #111), ported to the portable [`ListView`].
//!
//! The caller decides which tracks are visible (column browser + search, or
//! the folder selection) and hands them here; this module owns the list, the
//! row formatting and the sort indicator. Formatting and ordering come from
//! `emusic-ui` (the shared column definitions and `sort::compare`), so nothing
//! is duplicated per view.
//!
//! The portable list paints its own cells from a [`ListModel`], so the two
//! frontend-drawn columns (the star and the play marker) are model columns too:
//! their glyphs are static strings chosen from the row's state, and the audio
//! star/play behaviour is reached from the row context menu rather than a
//! per-cell click (the portable `ListView` has no cell-click hook; tracked as a
//! follow-up to add one).

use std::cell::Cell;
use std::rc::Rc;

use emusic_ui::library_api::TrackInfo;
use emusic_ui::state::Command;
use emusic_ui::views::track_table::columns::{self, ColumnId};
use emusic_ui::views::track_table::sort::{self, SortState};
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Column, Fill, ListModel, ListView, SortDirection};

use crate::app::Msg;

/// The frontend-drawn star toggle column, first in the table (#243). The
/// shared `emusic-ui` column list never mentions it: only the view knows how
/// to draw it, so the data columns stay index-compatible with
/// `columns::COLUMNS` at offset [`COLUMNS_OFFSET`].
pub(crate) const STAR_COLUMN: usize = 0;
/// The first data column (Title): the star and play-marker (#279) columns
/// shift every shared column right by two.
const COLUMNS_OFFSET: usize = 2;
/// The star column's width, in design units.
const STAR_COLUMN_WIDTH: f32 = 24.0;
/// The play marker column's width, in design units.
const PLAY_COLUMN_WIDTH: f32 = 20.0;

/// The data columns in table order: Title first, then the shared `COLUMNS`.
const DATA_COLUMNS: [ColumnId; 10] = [
    ColumnId::Title,
    ColumnId::Artist,
    ColumnId::Album,
    ColumnId::Year,
    ColumnId::Genre,
    ColumnId::Time,
    ColumnId::Format,
    ColumnId::Plays,
    ColumnId::LastPlayed,
    ColumnId::File,
];

/// The star cell's glyph: a filled star when starred, an outline otherwise.
pub(crate) fn star_glyph(starred: bool) -> &'static str {
    if starred { "\u{2605}" } else { "\u{2606}" }
}

/// A context-menu action on a track row.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    Play,
    PlayNext,
    AddToQueue,
    ToggleStar,
    CopyPath,
    OpenFileLocation,
    /// Open the track's Properties dialog (#280). Not ported yet.
    Properties,
    /// Open the track's tag editor (#278). Not ported yet.
    EditTags,
}

/// One row: the track plus its pre-formatted cell texts.
pub(crate) struct TrackRow {
    pub(crate) track: TrackInfo,
    /// The starred flag, mutable so a context-menu star can repaint the glyph
    /// without rebuilding the whole model.
    starred: Cell<bool>,
    /// The data cell texts in [`DATA_COLUMNS`] order.
    cells: [String; DATA_COLUMNS.len()],
}

impl TrackRow {
    fn new(track: &TrackInfo) -> Self {
        let mut cells = std::array::from_fn(|_| String::new());
        for (slot, id) in cells.iter_mut().zip(DATA_COLUMNS) {
            *slot = id.cell(track).into_owned();
        }
        Self {
            track: track.clone(),
            starred: Cell::new(track.starred),
            cells,
        }
    }

    /// Flips this row's star and returns the track id, so a star action can
    /// update the glyph and queue [`Command::ToggleStarred`].
    pub(crate) fn flip_star(&self) -> u64 {
        self.starred.set(!self.starred.get());
        self.track.id
    }
}

/// The list's model: the rows in display order plus the shared playing id.
struct MusicModel {
    rows: Rc<Vec<TrackRow>>,
    playing: Rc<Cell<Option<u64>>>,
}

impl ListModel for MusicModel {
    fn rows(&self) -> usize {
        self.rows.len()
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        let row = self.rows.get(row)?;
        Some(match column {
            STAR_COLUMN => star_glyph(row.starred.get()),
            1 => {
                if self.playing.get() == Some(row.track.id) {
                    "\u{25B6}"
                } else {
                    ""
                }
            }
            _ => row
                .cells
                .get(column - COLUMNS_OFFSET)
                .map_or("", String::as_str),
        })
    }
}

/// A virtual track table shared by the music-style views.
pub struct TrackView {
    list: ListView<Msg>,
    ui: Ui<Msg>,
    rows: Rc<Vec<TrackRow>>,
    playing: Rc<Cell<Option<u64>>>,
    /// Column index and direction currently showing a sort arrow, if any.
    indicator: Cell<Option<(usize, bool)>>,
}

impl TrackView {
    /// Creates the table and its (empty) virtual list.
    pub fn new(ui: &Ui<Msg>) -> TrackView {
        let playing = Rc::new(Cell::new(None));
        let mut list = ListView::new(ui, Rect::default(), &[])
            .expect("create track list")
            .multi_select(true);
        list = list
            .add_column(Column::new("", dip(STAR_COLUMN_WIDTH)).centered())
            .add_column(Column::new("", dip(PLAY_COLUMN_WIDTH)).centered())
            .column("Title", Fill);
        for spec in columns::COLUMNS {
            list = list.column(spec.label, dip(spec.initial_width));
        }
        let list = list
            .on_activate(|row| Some(Msg::PlayRow(row)))
            .on_sort(|column| Some(Msg::SortColumn(column)))
            .on_context(|row| Some(Msg::ContextRow(row)));

        let rows = Rc::new(Vec::new());
        list.set_model(MusicModel {
            rows: Rc::clone(&rows),
            playing: Rc::clone(&playing),
        });
        TrackView {
            list,
            ui: ui.clone(),
            rows,
            playing,
            indicator: Cell::new(None),
        }
    }

    /// The list's node, for the layout.
    pub fn id(&self) -> xui::xui_core::backend::WidgetId {
        self.list.id()
    }

    /// Moves/resizes the table.
    pub fn set_bounds(&self, rect: Rect) {
        self.ui.apply_moves(&[(self.list.id(), rect)]);
    }

    /// Shows or hides the table.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.list.id(), visible);
    }

    /// Rebuilds the model from `tracks`, sorted by `sort_state`, and updates
    /// the sort indicator.
    pub fn set_rows(&mut self, tracks: &[&TrackInfo], sort_state: SortState) {
        let mut tracks: Vec<&TrackInfo> = tracks.to_vec();
        apply_sort(&mut tracks, sort_state);
        self.rows = Rc::new(tracks.iter().map(|track| TrackRow::new(track)).collect());
        self.list.set_model(MusicModel {
            rows: Rc::clone(&self.rows),
            playing: Rc::clone(&self.playing),
        });
        self.show_sort_indicator(sort_state);
    }

    /// Repaints the playing-row highlight when it changed.
    pub fn sync_playing(&self, playing_id: Option<u64>) {
        if self.playing.get() != playing_id {
            self.playing.set(playing_id);
            self.ui.invalidate(self.list.id());
        }
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<Command> {
        let row = self.rows.as_slice().get(index)?;
        Some(Command::play_track(row.track.id, self.ids()))
    }

    /// Flips the star for `index`, repaints that row and returns the command
    /// to persist it.
    pub fn toggle_star(&self, index: usize) -> Option<Command> {
        let row = self.rows.as_slice().get(index)?;
        let id = row.flip_star();
        self.ui.invalidate(self.list.id());
        Some(Command::ToggleStarred(id))
    }

    /// The track at `index`, for a context action that needs its data.
    pub fn track(&self, index: usize) -> Option<&TrackInfo> {
        self.rows.get(index).map(|row| &row.track)
    }

    fn ids(&self) -> Vec<u64> {
        self.rows.iter().map(|row| row.track.id).collect()
    }

    fn show_sort_indicator(&self, sort_state: SortState) {
        let wanted = sort_state
            .key
            .and_then(|id| column_index(id).map(|index| (index, sort_state.ascending)));
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

/// Sorts `tracks` in place by the state's key, if any.
fn apply_sort(tracks: &mut [&TrackInfo], sort_state: SortState) {
    let Some(key) = sort_state.key else {
        return;
    };
    tracks.sort_by(|a, b| {
        let order = sort::compare(a, b, key);
        if sort_state.ascending {
            order
        } else {
            order.reverse()
        }
    });
}

/// The list column index for a [`ColumnId`] (0 = star, 1 = play marker,
/// 2 = Title, then `COLUMNS`).
pub(crate) fn column_index(id: ColumnId) -> Option<usize> {
    let index = DATA_COLUMNS.iter().position(|candidate| *candidate == id)?;
    Some(index + COLUMNS_OFFSET)
}

/// The [`ColumnId`] for a list column index, or `None` for the star and play
/// columns.
pub fn column_id(index: usize) -> Option<ColumnId> {
    DATA_COLUMNS
        .get(index.checked_sub(COLUMNS_OFFSET)?)
        .copied()
}

/// Runs a context action, returning the command to queue.
///
/// The clipboard and Explorer actions are not ported yet (they need a portable
/// clipboard/host seam; follow-up to #371/#376), so they return `None` after
/// being logged.
pub fn run_context_action(action: ContextAction, track: &TrackInfo) -> Option<Command> {
    match action {
        ContextAction::Play => Some(Command::play_track(track.id, Vec::new())),
        ContextAction::PlayNext => Some(Command::PlayTrackNext(track.id)),
        ContextAction::AddToQueue => Some(Command::QueueTrack(track.id)),
        ContextAction::ToggleStar => Some(Command::ToggleStarred(track.id)),
        ContextAction::CopyPath => {
            tracing::debug!(path = %track.path, "copy-path is not ported yet");
            None
        }
        ContextAction::OpenFileLocation => {
            tracing::debug!(path = %track.path, "open-file-location is not ported yet");
            None
        }
        ContextAction::Properties => None,
        ContextAction::EditTags => Some(Command::OpenTagEditor(track.id)),
    }
}
