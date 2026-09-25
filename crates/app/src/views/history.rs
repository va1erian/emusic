//! Win32 History view (#246): recorded plays grouped by day, newest first, in
//! a virtual (owner-data) [`ListView`] whose day headers are ordinary rows.
//!
//! The day grouping comes from the shared
//! [`emusic_ui::views::history`](emusic_ui::views::history) module; this module
//! only owns the native list, the row formatting and the "Clear history"
//! confirmation, mapping the list's events to the shell's [`Command`]s.
//! win32ui has no grouped `ListView`, so a flat list with header rows stands
//! in — the same shape the track table uses.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use emusic_ui::library_api::{HistoryEntry, LibraryDataSource, format_minutes_ago};
use emusic_ui::state::Command;
use emusic_ui::views::history::{self, Row};
use emusic_ui::views::track_table::columns;
use win32ui::prelude::*;
use win32ui::{
    Button, Control, Fill, Label, Layout, ListModel, ListView, Menu, RowStyle, TaskDialog,
    TaskDialogIcon, column, dip, row,
};

use crate::app::Msg;
use crate::views::track_table::ContextAction;

/// The "N plays" / "Clear history" band height, in design units.
const HEADER_HEIGHT: f32 = 28.0;
/// Nominal control heights, used to centre the band's controls vertically.
const LABEL_HEIGHT: f32 = 18.0;
const BUTTON_HEIGHT: f32 = 24.0;
/// Width of the "Clear history" button, in design units.
const CLEAR_WIDTH: f32 = 110.0;
/// Column widths for the history list, in design units.
const TIME_WIDTH: f32 = 110.0;
const ARTIST_WIDTH: f32 = 180.0;
const LISTENED_WIDTH: f32 = 90.0;
const STATUS_WIDTH: f32 = 90.0;

/// One row of the native history list: a day header or a recorded play.
enum HistoryRow {
    /// A day group header, e.g. "Today".
    Day(String),
    /// A recorded play.
    Entry(HistoryPlay),
}

impl HistoryRow {
    /// The cell text for `column` (0 = Time, then Title, Artist, Listened,
    /// Status). A day header only fills the first column.
    fn text(&self, column: usize) -> &str {
        match self {
            Self::Day(label) => {
                if column == 0 {
                    label.as_str()
                } else {
                    ""
                }
            }
            Self::Entry(play) => match column {
                0 => &play.time_text,
                1 => &play.title,
                2 => &play.artist,
                3 => &play.listened_text,
                4 => play.status,
                _ => "",
            },
        }
    }

    /// Whether this row's track is the one currently playing.
    fn is_playing(&self, playing_id: Option<u64>) -> bool {
        matches!(self, Self::Entry(play) if Some(play.track_id) == playing_id)
    }
}

/// One play, pre-formatted for the virtual list.
struct HistoryPlay {
    /// The `plays` row id, for removing this entry.
    entry_id: i64,
    /// The played track's library id, for playback.
    track_id: u64,
    time_text: String,
    title: String,
    artist: String,
    listened_text: String,
    status: &'static str,
}

impl HistoryPlay {
    /// Formats one entry; `playing_id` decides whether an in-progress play
    /// reads "Playing".
    fn new(entry: &HistoryEntry, now: i64, playing_id: Option<u64>) -> Self {
        let is_playing = playing_id == Some(entry.track_id);
        let minutes = ((now - entry.played_at).max(0) / 60) as u32;
        Self {
            entry_id: entry.id,
            track_id: entry.track_id,
            time_text: format_minutes_ago(minutes),
            title: text_or(entry.title.as_str(), "(unknown title)"),
            artist: text_or(entry.artist.as_str(), "(unknown artist)"),
            listened_text: columns::format_duration(Duration::from_millis(u64::from(
                entry.played_ms,
            ))),
            status: status_label(entry.finished, entry.completed, is_playing),
        }
    }
}

/// The owner-data model: the flattened rows in display order.
struct HistoryModel {
    rows: Rc<Vec<HistoryRow>>,
}

impl ListModel for HistoryModel {
    type Item = HistoryRow;

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, index: usize) -> Option<&HistoryRow> {
        self.rows.as_slice().get(index)
    }
}

/// The Win32 History view: the count/clear band over the grouped play list.
pub struct HistoryView {
    header: Label,
    clear: Button<Msg>,
    list: ListView<HistoryRow, Msg>,
    rows: Rc<Vec<HistoryRow>>,
    /// The playing track id, shared with the `row_style` closure so the
    /// highlight follows playback without rebuilding the model.
    playing: Rc<Cell<Option<u64>>>,
    context: Menu<Msg>,
    context_row: Cell<Option<usize>>,
    /// Whether the list has been built at least once.
    built: Cell<bool>,
}

impl HistoryView {
    /// Creates the band's controls and the (empty) grouped list.
    pub fn new(ui: &mut Ui<Msg>) -> Result<Self> {
        let header = Label::new(ui, Rect::default(), "0 plays")?;
        let clear = Button::new(ui, "Clear history")?.on_click(|| Some(Msg::HistoryClear));
        clear.set_enabled(false);

        let playing = Rc::new(Cell::new(None));
        let playing_for_style = Rc::clone(&playing);
        let list = ListView::new(ui)?
            .row_style(move |row: &HistoryRow| {
                if matches!(row, HistoryRow::Day(_)) || row.is_playing(playing_for_style.get()) {
                    RowStyle::new().bold(true)
                } else {
                    RowStyle::default()
                }
            })
            .column("Time", dip(TIME_WIDTH), |row: &HistoryRow| row.text(0))
            .column("Title", Fill, |row: &HistoryRow| row.text(1))
            .column("Artist", dip(ARTIST_WIDTH), |row: &HistoryRow| row.text(2))
            .column("Listened", dip(LISTENED_WIDTH), |row: &HistoryRow| {
                row.text(3)
            })
            .column("Status", dip(STATUS_WIDTH), |row: &HistoryRow| row.text(4))
            .on_activate(|row| Some(Msg::PlayRow(row)))
            .on_context(|row| Some(Msg::ContextRow(row)));

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
            .item("Remove from history", None, || {
                Msg::ContextAction(ContextAction::RemoveHistory)
            })
            .separator()
            .item("Edit tags…", None, || {
                Msg::ContextAction(ContextAction::EditTags)
            })
            .item("Properties…", None, || {
                Msg::ContextAction(ContextAction::Properties)
            });

        Ok(Self {
            header,
            clear,
            list,
            rows: Rc::new(Vec::new()),
            playing,
            context,
            context_row: Cell::new(None),
            built: Cell::new(false),
        })
    }

    /// Applies the current appearance metrics and zebra flag (#309).
    pub fn apply_appearance(&self) {
        crate::appearance::apply_list(&self.list);
    }

    /// Refreshes the list from the library. `rebuild` is set when the library
    /// (and so the history) changed; the list is also rebuilt on the first
    /// sync and whenever the playing track changes, because a play's status
    /// cell reads "Playing" only for the current track.
    pub fn sync(
        &mut self,
        library: &dyn LibraryDataSource,
        playing_id: Option<u64>,
        rebuild: bool,
    ) {
        if !self.built.get() || rebuild || self.playing.get() != playing_id {
            self.rebuild(library, playing_id);
            self.built.set(true);
        }
    }

    /// Rebuilds the model from the library's history and updates the band.
    fn rebuild(&mut self, library: &dyn LibraryDataSource, playing_id: Option<u64>) {
        let entries = library.history();
        let now = unix_now();
        let rows = build_rows(entries, now, playing_id);
        self.header.set_text(&format!("{} plays", entries.len()));
        self.clear.set_enabled(!entries.is_empty());
        self.playing.set(playing_id);
        self.rows = Rc::new(rows);
        self.list.set_model(HistoryModel {
            rows: Rc::clone(&self.rows),
        });
    }

    /// The command to replay `index` in the context of the whole visible list.
    /// Day-header rows are not plays, so activating one does nothing.
    pub fn activate(&self, index: usize) -> Option<Command> {
        let play = self.play_at(index)?;
        Some(Command::play_track(play.track_id, self.context_ids()))
    }

    /// Runs a context action on the row that opened the menu.
    pub fn run_context(&self, action: ContextAction) -> Option<Command> {
        let play = self
            .context_row
            .get()
            .and_then(|index| self.play_at(index))?;
        match action {
            ContextAction::Play => Some(Command::play_track(play.track_id, self.context_ids())),
            ContextAction::PlayNext => Some(Command::PlayTrackNext(play.track_id)),
            ContextAction::AddToQueue => Some(Command::QueueTrack(play.track_id)),
            ContextAction::ToggleStar => Some(Command::ToggleStarred(play.track_id)),
            ContextAction::RemoveHistory => Some(Command::HistoryRemove(play.entry_id)),
            ContextAction::CopyPath | ContextAction::OpenFileLocation => None,
            // Handled by the app, which shows the modal dialog itself.
            ContextAction::Properties => None,
            ContextAction::EditTags => Some(Command::OpenTagEditor(play.track_id)),
        }
    }

    /// The library track id of the play whose context menu is open, if any.
    pub fn context_track_id(&self) -> Option<u64> {
        self.context_row
            .get()
            .and_then(|index| self.play_at(index))
            .map(|play| play.track_id)
    }

    /// Whether `row` is a play (rather than a day header), so the app knows
    /// whether to open the context menu for it.
    pub fn is_entry_row(&self, row: usize) -> bool {
        self.play_at(row).is_some()
    }

    /// The play at `index`, or `None` for a day header / past the end.
    fn play_at(&self, index: usize) -> Option<&HistoryPlay> {
        match self.rows.as_slice().get(index)? {
            HistoryRow::Entry(play) => Some(play),
            HistoryRow::Day(_) => None,
        }
    }

    /// The play ids of the whole visible list, in display order (day headers
    /// skipped), for [`Command::play_track`]'s queue context.
    fn context_ids(&self) -> Vec<u64> {
        self.rows
            .iter()
            .filter_map(|row| match row {
                HistoryRow::Entry(play) => Some(play.track_id),
                HistoryRow::Day(_) => None,
            })
            .collect()
    }

    /// Remembers the row the context menu was opened on.
    pub fn set_context_row(&self, row: usize) {
        self.context_row.set(Some(row));
    }

    /// The list's row context menu.
    pub fn context_menu(&self) -> &Menu<Msg> {
        &self.context
    }

    /// Shows the confirm dialog for clearing the whole history, returning
    /// whether the user chose to clear. Returns `false` when the dialog cannot
    /// be shown at all (e.g. Common Controls v6 is unavailable).
    pub fn confirm_clear(&self, ui: &Ui<Msg>) -> bool {
        let dialog = TaskDialog::new("Clear play history?")
            .content(
                "This removes every recorded play, including the most-played \
                 rankings. Per-track play counts are kept. This cannot be undone.",
            )
            .buttons([("Clear history", true), ("Cancel", false)])
            .default(false)
            .icon(TaskDialogIcon::Warning);
        match dialog.show(ui) {
            Ok((clear, _)) => clear,
            Err(error) => {
                tracing::warn!(%error, "clear-history confirmation unavailable");
                false
            }
        }
    }

    /// Shows or hides the whole view (its band and list).
    pub fn set_visible(&self, visible: bool) {
        self.header.set_visible(visible);
        self.clear.set_visible(visible);
        self.list.set_visible(visible);
    }

    /// The count/clear band over the grouped play list.
    pub fn layout(&self) -> Layout {
        column![
            row![
                centered(&self.header, LABEL_HEIGHT).fill(1),
                centered(&self.clear, BUTTON_HEIGHT).width(dip(CLEAR_WIDTH)),
            ]
            .spacing(dip(6.0))
            .height(dip(HEADER_HEIGHT)),
            self.list.fill(1),
        ]
    }
}

impl AsControl for HistoryView {
    fn control(&self) -> &Control {
        self.list.control()
    }
}

/// Wraps a band control so it is centred vertically in the [`HEADER_HEIGHT`]
/// band: symmetric top/bottom margins shrink its area to `height`.
fn centered(control: &impl AsControl, height: f32) -> Layout {
    let pad = ((HEADER_HEIGHT - height) / 2.0).max(0.0);
    Layout::row()
        .item(control.fill(1))
        .margins(Insets::new(dip(0.0), dip(pad), dip(0.0), dip(pad)))
}

/// Flattens the shared grouping into pre-formatted native rows.
fn build_rows(entries: &[HistoryEntry], now: i64, playing_id: Option<u64>) -> Vec<HistoryRow> {
    history::build_rows(entries, now)
        .into_iter()
        .map(|row| match row {
            Row::Day(label) => HistoryRow::Day(label),
            Row::Entry(entry) => HistoryRow::Entry(HistoryPlay::new(entry, now, playing_id)),
        })
        .collect()
}

/// The current Unix time, for grouping and relative ages. `0` on a clock
/// before the epoch (never in practice).
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `value`, or `fallback` when it is empty.
fn text_or(value: &str, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

/// Status text for a history row: a play that is still in progress reads
/// "Playing" while its track is the current one, otherwise it is classified
/// by the player's completion verdict once it has finished.
fn status_label(finished: bool, completed: bool, is_playing: bool) -> &'static str {
    if !finished && is_playing {
        "Playing"
    } else if completed {
        "Completed"
    } else {
        "Skipped"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64, track_id: u64, played_at: i64) -> HistoryEntry {
        HistoryEntry {
            id,
            track_id,
            title: format!("Track {track_id}"),
            artist: "Artist".to_string(),
            played_at,
            played_ms: 90_000,
            finished: true,
            completed: true,
        }
    }

    #[test]
    fn status_label_prioritises_a_live_play_then_completion() {
        assert_eq!(status_label(false, false, true), "Playing");
        assert_eq!(status_label(false, false, false), "Skipped");
        assert_eq!(status_label(true, true, true), "Completed");
        assert_eq!(status_label(true, false, false), "Skipped");
    }

    #[test]
    fn build_rows_flattens_entries_under_day_headers() {
        const DAY: i64 = 86_400;
        let now = 1_000_000_000;
        let entries = vec![
            entry(1, 10, now),
            entry(2, 20, now - DAY),
            entry(3, 30, now - DAY - 60),
        ];
        let rows = build_rows(&entries, now, Some(20));

        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].text(0), "Today");
        assert_eq!(rows[0].text(1), "", "a header only fills the first column");
        assert_eq!(rows[2].text(0), "Yesterday", "the second day header");
        assert_eq!(rows[3].text(1), "Track 20");
        assert_eq!(rows[3].text(4), "Completed", "a finished play's status");
        assert!(rows[3].is_playing(Some(20)));
        assert!(!rows[1].is_playing(Some(20)));
    }

    #[test]
    fn an_unfinished_current_play_reads_playing() {
        let now = 1_000_000_000;
        let mut play = entry(1, 10, now);
        play.finished = false;
        play.completed = false;

        assert_eq!(HistoryPlay::new(&play, now, Some(10)).status, "Playing");
        assert_eq!(HistoryPlay::new(&play, now, None).status, "Skipped");
    }

    #[test]
    fn text_or_falls_back_on_empty_fields() {
        assert_eq!(text_or("", "(unknown)"), "(unknown)");
        assert_eq!(text_or("Title", "(unknown)"), "Title");
    }
}
