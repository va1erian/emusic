//! The shared Win32 track table (#107, #111): a virtual (owner-data)
//! `ListView` over a pre-filtered, pre-sorted slice of tracks.
//!
//! Both the Music and Folders views reuse it: the caller decides which tracks
//! are visible (column browser + search, or the folder selection) and hands
//! them here; this module owns the native list, the row formatting, the context
//! menu and the sort indicator. Formatting and ordering come from `emusic-ui`
//! (the shared column definitions and `sort::compare`), so nothing is
//! duplicated per view.

use std::cell::Cell;
use std::rc::Rc;

use emusic_ui::library_api::{TrackInfo, format_minutes_ago};
use emusic_ui::state::Command;
use emusic_ui::views::track_table::columns::{self, ColumnId};
use emusic_ui::views::track_table::sort::{self, SortState};
use win32ui::dnd::DropEffects;
use win32ui::prelude::*;
use win32ui::{Column, ColumnWidth, Fill, ListModel, ListView, RowStyle, SortDirection, dip};

use crate::app::Msg;

/// The frontend-drawn star toggle column, first in the table (#243). The
/// shared `emusic-ui` column list never mentions it: only the view knows
/// how to draw and click it, so the data columns stay index-compatible with
/// `columns::COLUMNS` at offset [`COLUMNS_OFFSET`].
pub(crate) const STAR_COLUMN: usize = 0;
/// The first data column (Title): the star and play-marker (#279) columns shift
/// every shared column right by two.
const COLUMNS_OFFSET: usize = 2;
/// The star column's width, in design units.
const STAR_COLUMN_WIDTH: f32 = 24.0;
/// The play marker column's width, in design units.
const PLAY_COLUMN_WIDTH: f32 = 20.0;

/// The star cell's glyph: a filled star when starred, an outline otherwise,
/// matching the now-playing summary.
pub(crate) fn star_glyph(starred: bool) -> &'static str {
    if starred { "\u{2605}" } else { "\u{2606}" }
}

/// Whether a table is a plain list or also a reorder drop target (#476).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TableMode {
    /// No drop target: the table only starts drags.
    Plain,
    /// The list accepts row drops for a manual reorder (the Playlist view).
    Reorder,
}

/// Builds the star toggle column: centred, not resizable, its glyph tinted
/// with the theme accent when starred and dim otherwise.
pub(crate) fn star_column() -> Column<TrackRow> {
    Column::new("", dip(STAR_COLUMN_WIDTH), |row: &TrackRow| {
        star_glyph(row.starred.get())
    })
    .centered()
    .resizable(false)
    .cell_color(|row, theme| {
        Some(if row.starred.get() {
            theme.accent
        } else {
            theme.text_secondary
        })
    })
}

/// Builds the play marker column: a play glyph in the accent colour on the row
/// of the track `playing` holds, empty elsewhere.
pub(crate) fn play_column(playing: Rc<Cell<Option<u64>>>) -> Column<TrackRow> {
    Column::new("", dip(PLAY_COLUMN_WIDTH), move |row: &TrackRow| {
        if playing.get() == Some(row.track.id) {
            "\u{25B6}"
        } else {
            ""
        }
    })
    .centered()
    .resizable(false)
    .cell_color(|_, theme| Some(theme.accent))
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
    /// Remove the row's playback-history entry (History view only).
    RemoveHistory,
    /// Open the track's Properties dialog (#280).
    Properties,
    /// Open the track's tag editor (#278).
    EditTags,
}

/// One row: the track plus its pre-formatted numeric cells. Shared with the
/// albums grid's track list.
pub(crate) struct TrackRow {
    pub(crate) track: TrackInfo,
    /// The starred flag, mutable after build so a star-cell click repaints the
    /// glyph without rebuilding the whole model.
    starred: Cell<bool>,
    year_text: String,
    time_text: String,
    plays_text: String,
    last_played_text: String,
}

impl TrackRow {
    pub(crate) fn new(track: &TrackInfo) -> Self {
        Self {
            track: track.clone(),
            starred: Cell::new(track.starred),
            year_text: track.year.map(|year| year.to_string()).unwrap_or_default(),
            time_text: columns::format_duration(track.duration),
            plays_text: track.play_count.to_string(),
            last_played_text: track
                .last_played_minutes_ago
                .map(format_minutes_ago)
                .unwrap_or_default(),
        }
    }

    /// Flips this row's star and returns the track id, so a cell click can
    /// update the glyph and queue [`Command::ToggleStarred`].
    pub(crate) fn flip_star(&self) -> u64 {
        self.starred.set(!self.starred.get());
        self.track.id
    }

    /// The cell text for `column` (0 = Title, then `columns::COLUMNS`).
    pub(crate) fn text(&self, column: usize) -> &str {
        match column {
            0 => columns::title_text(&self.track),
            1 => columns::artist_text(&self.track),
            2 => &self.track.album,
            3 => &self.year_text,
            4 => &self.track.genre,
            5 => &self.time_text,
            6 => &self.track.format,
            7 => &self.plays_text,
            8 => &self.last_played_text,
            _ => &self.track.path,
        }
    }
}

/// The list's owner-data model: the rows in display order. Shared with the
/// albums grid's track list.
pub(crate) struct MusicModel {
    pub(crate) rows: Rc<Vec<TrackRow>>,
}

impl ListModel for MusicModel {
    type Item = TrackRow;

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, index: usize) -> Option<&TrackRow> {
        self.rows.as_slice().get(index)
    }
}

/// A virtual track table shared by the Music and Folders views.
pub struct TrackView {
    list: ListView<TrackRow, Msg>,
    rows: Rc<Vec<TrackRow>>,
    /// The playing track id, shared with the `row_style` closure so the
    /// highlight follows playback without rebuilding the model.
    playing: Rc<Cell<Option<u64>>>,
    context_row: Cell<Option<usize>>,
    /// The rows the current drag started from (#476), so a drop back onto the
    /// same list can reorder them.
    dragged: Cell<Vec<usize>>,
    /// Whether a drop may reorder the list: false while a sort column is
    /// active, so the native insert marker is not shown (#476).
    reorder_ok: Rc<Cell<bool>>,
    /// Column index and direction currently showing a sort arrow, if any.
    indicator: Cell<Option<(usize, bool)>>,
}

impl TrackView {
    /// Creates the table and its (empty) virtual list. A [`TableMode::Reorder`]
    /// table also accepts row drops for a manual reorder.
    pub fn new(ui: &mut Ui<Msg>, mode: TableMode) -> Result<Self> {
        let playing = Rc::new(Cell::new(None));
        let playing_for_style = Rc::clone(&playing);

        let mut list = ListView::new(ui)?
            .multi_select(true)
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
                (column == STAR_COLUMN).then_some(Msg::ToggleStarRow(row))
            })
            .on_activate(|row| Some(Msg::PlayRow(row)))
            .on_sort(|column| Some(Msg::SortColumn(column)))
            .on_context(|row| Some(Msg::ContextRow(row)))
            .on_begin_drag(|rows, _| Some(Msg::TrackBeginDrag(rows.to_vec())));
        for column in columns::COLUMNS {
            let id = column.id;
            list = list.column(
                column.label,
                ColumnWidth::Fixed(dip(column.initial_width)),
                move |row: &TrackRow| cell_text(row, id),
            );
        }

        let reorder_ok = Rc::new(Cell::new(mode == TableMode::Reorder));
        if mode == TableMode::Reorder {
            let allowed = Rc::clone(&reorder_ok);
            list = list
                .drop_filter(move |data| data.has_payload() && allowed.get())
                .on_drop(|drop| Some(Msg::TrackDrop(drop)))?;
        }

        Ok(Self {
            list,
            rows: Rc::new(Vec::new()),
            playing,
            context_row: Cell::new(None),
            dragged: Cell::new(Vec::new()),
            reorder_ok,
            indicator: Cell::new(None),
        })
    }

    /// Applies the current appearance metrics and zebra flag: the row font,
    /// row height and striping (#309).
    pub fn apply_appearance(&self) {
        crate::appearance::apply_list(&self.list);
    }

    /// Rebuilds the model from `tracks`, sorted by `sort_state`, and updates
    /// the sort indicator.
    pub fn set_rows(&mut self, tracks: &[&TrackInfo], sort_state: SortState) {
        let mut tracks: Vec<&TrackInfo> = tracks.to_vec();
        self.apply_sort(&mut tracks, sort_state);
        self.rows = Rc::new(tracks.iter().map(|track| TrackRow::new(track)).collect());
        self.list.set_model(MusicModel {
            rows: Rc::clone(&self.rows),
        });
        self.show_sort_indicator(sort_state);
    }

    /// Repaints the playing-row highlight when it changed.
    pub fn sync_playing(&self, playing_id: Option<u64>) {
        if self.playing.get() != playing_id {
            self.playing.set(playing_id);
            let len = self.rows.len();
            self.list.rows_changed(0..len);
        }
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<Command> {
        let row = self.rows.as_slice().get(index)?;
        Some(Command::play_track(row.track.id, self.context_ids()))
    }

    /// Flips the star for `index`, repaints that row and returns the command
    /// to persist it. The cell click that raised this never moved the
    /// selection, so the row stays put.
    pub fn toggle_star(&self, index: usize) -> Option<Command> {
        let row = self.rows.as_slice().get(index)?;
        let id = row.flip_star();
        self.list.rows_changed(index..index + 1);
        Some(Command::ToggleStarred(id))
    }

    /// Runs a context action on the row that opened the menu.
    pub fn run_context(&self, action: ContextAction, hwnd: win32ui::Hwnd) -> Option<Command> {
        let row = self
            .context_row
            .get()
            .and_then(|index| self.rows.as_slice().get(index))?;
        run_context_action(action, &row.track, hwnd)
    }

    /// The track whose context menu is open, if any (for the Properties
    /// dialog, which the app shows itself rather than as a [`Command`]).
    pub fn context_track(&self) -> Option<TrackInfo> {
        self.context_row
            .get()
            .and_then(|index| self.rows.as_slice().get(index))
            .map(|row| row.track.clone())
    }

    pub fn set_context_row(&self, row: usize) {
        self.context_row.set(Some(row));
    }

    /// Starts dragging `rows`: records them for a possible reorder and runs the
    /// drag with their track ids as the payload (#476). Blocks until the drag
    /// ends, so call it from the [`Msg::TrackBeginDrag`] handler.
    pub fn begin_drag(&self, rows: &[usize]) {
        self.dragged.set(rows.to_vec());
        let tracks: Vec<u64> = rows
            .iter()
            .filter_map(|&row| self.rows.as_slice().get(row))
            .map(|row| row.track.id)
            .collect();
        if tracks.is_empty() {
            return;
        }
        let payload = crate::dnd::encode_track_ids(&tracks);
        let _ = self.list.begin_drag(&payload, DropEffects::COPY, None);
    }

    /// The rows the current drag started from.
    pub fn dragged_rows(&self) -> Vec<usize> {
        self.dragged.replace(Vec::new())
    }

    /// Sets whether a drop may reorder the table (false while sorted).
    pub fn set_reorder_ok(&self, ok: bool) {
        self.reorder_ok.set(ok);
    }

    /// Queues adding the context row's selection to playlist `id` (#476): the
    /// whole selection when the row is part of it, else just that row.
    pub fn add_to_playlist(&self, id: u64) -> Option<Command> {
        let row = self.context_row.get()?;
        let selection = self.list.selection();
        let rows = if selection.contains(&row) {
            selection
        } else {
            vec![row]
        };
        let tracks: Vec<u64> = rows
            .iter()
            .filter_map(|&index| self.rows.as_slice().get(index))
            .map(|row| row.track.id)
            .collect();
        (!tracks.is_empty()).then_some(Command::AddToPlaylist { id, tracks })
    }

    fn context_ids(&self) -> Vec<u64> {
        self.rows.iter().map(|row| row.track.id).collect()
    }

    fn apply_sort(&self, tracks: &mut [&TrackInfo], sort_state: SortState) {
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

impl AsControl for TrackView {
    fn control(&self) -> &Control {
        self.list.control()
    }
}

/// The cell text for `id` within a row, using the shared column helpers.
pub(crate) fn cell_text(row: &TrackRow, id: ColumnId) -> &str {
    match id {
        ColumnId::Title => row.text(0),
        ColumnId::Artist => row.text(1),
        ColumnId::Album => row.text(2),
        ColumnId::Year => row.text(3),
        ColumnId::Genre => row.text(4),
        ColumnId::Time => row.text(5),
        ColumnId::Format => row.text(6),
        ColumnId::Plays => row.text(7),
        ColumnId::LastPlayed => row.text(8),
        ColumnId::File => row.text(9),
    }
}

/// The list column index for a [`ColumnId`] (0 = star, 1 = play marker,
/// 2 = Title, then `COLUMNS`).
pub(crate) fn column_index(id: ColumnId) -> Option<usize> {
    if id == ColumnId::Title {
        return Some(COLUMNS_OFFSET);
    }
    columns::COLUMNS
        .iter()
        .position(|column| column.id == id)
        .map(|index| index + COLUMNS_OFFSET + 1)
}

/// The [`ColumnId`] for a list column index, or `None` for the star column.
pub fn column_id(index: usize) -> Option<ColumnId> {
    if index == COLUMNS_OFFSET {
        return Some(ColumnId::Title);
    }
    columns::COLUMNS
        .get(index.checked_sub(COLUMNS_OFFSET + 1)?)
        .map(|column| column.id)
}

/// Runs a context action, returning the command to queue (or executing the
/// clipboard/Explorer action directly).
pub fn run_context_action(
    action: ContextAction,
    track: &TrackInfo,
    hwnd: win32ui::Hwnd,
) -> Option<Command> {
    match action {
        ContextAction::Play => Some(Command::play_track(track.id, Vec::new())),
        ContextAction::PlayNext => Some(Command::PlayTrackNext(track.id)),
        ContextAction::AddToQueue => Some(Command::QueueTrack(track.id)),
        ContextAction::ToggleStar => Some(Command::ToggleStarred(track.id)),
        ContextAction::CopyPath => {
            let _ = win32ui::clipboard::set_text(hwnd, &track.path);
            None
        }
        ContextAction::OpenFileLocation => {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{}", track.path.replace('/', "\\")))
                .spawn();
            None
        }
        // Only the History view's own context menu raises this; the track
        // table has no history entry to remove.
        ContextAction::RemoveHistory => None,
        // Handled by the app, which shows the modal dialog itself.
        ContextAction::Properties => None,
        ContextAction::EditTags => Some(Command::OpenTagEditor(track.id)),
    }
}
