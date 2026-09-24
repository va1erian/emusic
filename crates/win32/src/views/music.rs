//! Win32 Music view (#107): a virtual (owner-data) `ListView` over the
//! library, filtered by the column browser and the top-bar search.
//!
//! All formatting and ordering comes from `emusic-ui` (the shared column
//! definitions and `sort::compare`); this module only owns the native list and
//! turns its events into [`Msg`]s. Rows are pre-formatted once so owner-data
//! requests never allocate.

use std::cell::Cell;
use std::rc::Rc;

use emusic_ui::library_api::{TrackInfo, format_minutes_ago};
use emusic_ui::state::{AppState, Command};
use emusic_ui::views::track_table::columns::{self, ColumnId};
use emusic_ui::views::track_table::sort::{self, SortState};
use win32ui::prelude::*;
use win32ui::{ColumnWidth, Fill, ListView, Menu, RowStyle, SortDirection, dip};

use crate::app::Msg;

/// A context-menu action on a track row.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    Play,
    PlayNext,
    AddToQueue,
    ToggleStar,
    CopyPath,
    OpenFileLocation,
}

/// One row: the track plus its pre-formatted numeric cells.
struct TrackRow {
    track: TrackInfo,
    year_text: String,
    time_text: String,
    plays_text: String,
    last_played_text: String,
}

impl TrackRow {
    fn new(track: &TrackInfo) -> Self {
        Self {
            track: track.clone(),
            year_text: track.year.map(|year| year.to_string()).unwrap_or_default(),
            time_text: columns::format_duration(track.duration),
            plays_text: track.play_count.to_string(),
            last_played_text: track
                .last_played_minutes_ago
                .map(format_minutes_ago)
                .unwrap_or_default(),
        }
    }

    /// The cell text for `column` (0 = Title, then `columns::COLUMNS`).
    fn text(&self, column: usize) -> &str {
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

/// The list's owner-data model: the filtered rows in display order.
struct MusicModel {
    rows: Rc<Vec<TrackRow>>,
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

/// What the model was built from; the model is only rebuilt when it changes.
#[derive(PartialEq)]
struct Signature {
    track_count: usize,
    scan: bool,
    search_active: bool,
    search_count: Option<usize>,
    column_browser: emusic_ui::views::column_browser::ColumnBrowser,
}

/// The Music view: the library track table.
pub struct MusicView {
    list: ListView<TrackRow, Msg>,
    rows: Rc<Vec<TrackRow>>,
    /// The playing track id, shared with the `row_style` closure so the
    /// highlight follows playback without rebuilding the model.
    playing: Rc<Cell<Option<u64>>>,
    signature: Option<Signature>,
    context: Menu<Msg>,
    context_row: Cell<Option<usize>>,
    /// Column index and direction currently showing a sort arrow, if any.
    indicator: Cell<Option<(usize, bool)>>,
}

impl MusicView {
    /// Creates the view and its (empty) virtual list.
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
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
            .column("Title", Fill, |row: &TrackRow| row.text(0))
            .on_activate(|row| Some(Msg::PlayRow(row)))
            .on_sort(|column| Some(Msg::SortColumn(column)))
            .on_context(|row| Some(Msg::ContextRow(row)));
        for column in columns::COLUMNS {
            let id = column.id;
            list = list.column(
                column.label,
                ColumnWidth::Fixed(dip(column.initial_width)),
                move |row: &TrackRow| cell_text(row, id),
            );
        }

        let context = Menu::new()
            .item("Play", None, || Msg::ContextAction(ContextAction::Play))
            .item("Play next", None, || {
                Msg::ContextAction(ContextAction::PlayNext)
            })
            .item("Add to queue", None, || {
                Msg::ContextAction(ContextAction::AddToQueue)
            })
            .separator()
            .item("Star / Unstar", None, || {
                Msg::ContextAction(ContextAction::ToggleStar)
            })
            .separator()
            .item("Open file location", None, || {
                Msg::ContextAction(ContextAction::OpenFileLocation)
            })
            .item("Copy path", None, || {
                Msg::ContextAction(ContextAction::CopyPath)
            });

        Ok(Self {
            list,
            rows: Rc::new(Vec::new()),
            playing,
            signature: None,
            context,
            context_row: Cell::new(None),
            indicator: Cell::new(None),
        })
    }

    /// Rebuilds the model and playing highlight from the shell state. `changed`
    /// gates the expensive row rebuild.
    pub fn sync(
        &mut self,
        state: &AppState,
        library: &dyn emusic_ui::library_api::LibraryDataSource,
        search: &emusic_ui::search::SearchEngine,
        playing_id: Option<u64>,
        changed: emusic_ui::shell::Changes,
    ) {
        use emusic_ui::shell::Changes;

        let signature = Signature {
            track_count: library.track_count(),
            scan: library.is_scanning(),
            search_active: search.is_active(),
            search_count: search.match_count(),
            column_browser: state.music.browser.clone(),
        };
        let stale = self.signature.as_ref() != Some(&signature);
        if stale {
            let browser_changed = self
                .signature
                .as_ref()
                .is_none_or(|previous| previous.column_browser != signature.column_browser);
            if changed.intersects(Changes::LIBRARY | Changes::SEARCH) || browser_changed {
                self.rebuild(
                    library,
                    search,
                    &signature.column_browser,
                    state.music.table.sort,
                );
                self.signature = Some(signature);
            }
        }

        if self.playing.get() != playing_id {
            self.playing.set(playing_id);
            let len = self.rows.len();
            self.list.rows_changed(0..len);
        }

        self.show_sort_indicator(state.music.table.sort);
    }

    /// Applies the current sort order to the model (after a header click).
    pub fn resort(
        &mut self,
        state: &AppState,
        library: &dyn emusic_ui::library_api::LibraryDataSource,
        search: &emusic_ui::search::SearchEngine,
    ) {
        self.rebuild(
            library,
            search,
            &state.music.browser,
            state.music.table.sort,
        );
        self.show_sort_indicator(state.music.table.sort);
    }

    /// The command to play `index` in the context of the whole visible list.
    pub fn activate(&self, index: usize) -> Option<Command> {
        let row = self.rows.as_slice().get(index)?;
        Some(Command::play_track(row.track.id, self.context_ids()))
    }

    /// Runs a context action on the row that opened the menu.
    pub fn run_context(&self, action: ContextAction, hwnd: win32ui::Hwnd) -> Option<Command> {
        let row = self
            .context_row
            .get()
            .and_then(|index| self.rows.as_slice().get(index))?;
        run_context_action(action, &row.track, hwnd)
    }

    pub fn set_context_row(&self, row: usize) {
        self.context_row.set(Some(row));
    }

    pub fn context_menu(&self) -> &Menu<Msg> {
        &self.context
    }

    fn context_ids(&self) -> Vec<u64> {
        self.rows.iter().map(|row| row.track.id).collect()
    }

    fn rebuild(
        &mut self,
        library: &dyn emusic_ui::library_api::LibraryDataSource,
        search: &emusic_ui::search::SearchEngine,
        column_browser: &emusic_ui::views::column_browser::ColumnBrowser,
        sort_state: SortState,
    ) {
        let mut tracks: Vec<&TrackInfo> = library
            .tracks()
            .iter()
            .filter(|track| column_browser.matches(track))
            .filter(|track| search.is_match(track.id))
            .collect();
        self.apply_sort(&mut tracks, sort_state);
        self.rows = Rc::new(tracks.iter().map(|track| TrackRow::new(track)).collect());
        self.list.set_model(MusicModel {
            rows: Rc::clone(&self.rows),
        });
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

impl AsControl for MusicView {
    fn control(&self) -> &Control {
        self.list.control()
    }
}

/// The cell text for `id` within a row, using the shared column helpers.
fn cell_text(row: &TrackRow, id: ColumnId) -> &str {
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

/// The list column index for a [`ColumnId`] (0 = Title, then `COLUMNS`).
fn column_index(id: ColumnId) -> Option<usize> {
    if id == ColumnId::Title {
        return Some(0);
    }
    columns::COLUMNS
        .iter()
        .position(|column| column.id == id)
        .map(|index| index + 1)
}

/// The [`ColumnId`] for a list column index.
pub fn column_id(index: usize) -> Option<ColumnId> {
    if index == 0 {
        return Some(ColumnId::Title);
    }
    columns::COLUMNS.get(index - 1).map(|column| column.id)
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
    }
}
