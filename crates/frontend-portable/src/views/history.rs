//! The History view (#246), ported to the portable widget layer: recorded plays
//! grouped by day, newest first, in a model `ListView` whose day headers are
//! ordinary rows.
//!
//! The day grouping comes from the shared
//! [`emusic_ui::views::history`](emusic_ui::views::history) module; this module
//! only owns the list, the row formatting and the "Clear history" button,
//! mapping the list's events to the shell's [`Command`]s. The portable
//! `ListView` has no grouped mode, so a flat list with header rows stands in —
//! the same shape the track table uses.
//!
//! The original Win32 list drew day headers and the playing row in bold and
//! confirmed "Clear history" with a modal dialog; the portable list has no
//! per-row style hook and the dialogs are #376's, so both are follow-ups (the
//! button clears directly).

use std::cell::Cell;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use emusic_ui::library_api::{HistoryEntry, LibraryDataSource, format_minutes_ago};
use emusic_ui::search::SearchEngine;
use emusic_ui::state::Command;
use emusic_ui::views::filter;
use emusic_ui::views::history::{self, Row};
use emusic_ui::views::track_table::columns;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Button, Fill, HasText, Label, ListModel, ListView};

use crate::app::Msg;
use crate::make::make;
use xui::xui_core::arrange::{button, label, list};

/// The "N plays" / "Clear history" band height, in design units.
const HEADER_HEIGHT: f32 = 28.0;
/// Nominal button height, used to centre it in the band.
const BUTTON_HEIGHT: f32 = 24.0;
/// Width of the "Clear history" button, in design units.
const CLEAR_WIDTH: f32 = 110.0;
/// Horizontal inset of the band's contents, in design units.
const INSET: f32 = 8.0;
/// Column widths for the history list, in design units.
const TIME_WIDTH: f32 = 110.0;
const ARTIST_WIDTH: f32 = 180.0;
const LISTENED_WIDTH: f32 = 90.0;
const STATUS_WIDTH: f32 = 90.0;

/// One row of the history list: a day header or a recorded play.
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
    #[cfg(test)]
    fn is_playing(&self, playing_id: Option<u64>) -> bool {
        matches!(self, Self::Entry(play) if Some(play.track_id) == playing_id)
    }
}

/// One play, pre-formatted for the virtual list.
struct HistoryPlay {
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
            track_id: entry.track_id,
            time_text: format_minutes_ago(minutes),
            title: text_or(entry.title.as_str(), "(unknown title)"),
            artist: text_or(entry.artist.as_str(), "(unknown artist)"),
            listened_text: columns::format_duration(std::time::Duration::from_millis(u64::from(
                entry.played_ms,
            ))),
            status: status_label(entry.finished, entry.completed, is_playing),
        }
    }
}

/// The model: the flattened rows in display order.
struct HistoryModel {
    rows: Rc<Vec<HistoryRow>>,
}

impl ListModel for HistoryModel {
    fn rows(&self) -> usize {
        self.rows.len()
    }

    fn cell(&self, row: usize, column: usize) -> Option<&str> {
        Some(self.rows.get(row)?.text(column))
    }
}

/// The History view: the count/clear band over the grouped play list.
pub struct HistoryView {
    ui: Ui<Msg>,
    header: Label<Msg>,
    clear: Button<Msg>,
    list: ListView<Msg>,
    rows: Rc<Vec<HistoryRow>>,
    /// The playing track id last mirrored into the model.
    playing: Cell<Option<u64>>,
    /// Whether the list has been built at least once.
    built: Cell<bool>,
    /// The search revision the rows were last built for, so switching into the
    /// view after a query change rebuilds them.
    applied_search: Cell<u64>,
}

impl HistoryView {
    /// Creates the band's controls and the (empty) grouped list.
    pub fn new(ui: &Ui<Msg>) -> HistoryView {
        let header = make(ui, label("0 plays")).expect("create history header");
        let clear = make(ui, button("Clear history"))
            .expect("create clear-history button")
            .on_click(|| Some(Msg::HistoryClear));
        clear.set_enabled(false);

        let list = make(ui, list())
            .expect("create history list")
            .column("Time", dip(TIME_WIDTH))
            .column("Title", Fill)
            .column("Artist", dip(ARTIST_WIDTH))
            .column("Listened", dip(LISTENED_WIDTH))
            .column("Status", dip(STATUS_WIDTH))
            .on_activate(|row| Some(Msg::PlayRow(row)))
            .on_context(|row, at| Some(Msg::ContextRow(row, at)));

        HistoryView {
            ui: ui.clone(),
            header,
            clear,
            list,
            rows: Rc::new(Vec::new()),
            playing: Cell::new(None),
            built: Cell::new(false),
            applied_search: Cell::new(u64::MAX),
        }
    }

    /// Moves and sizes the whole view: the count/clear band on top, the list
    /// filling the rest.
    pub fn set_bounds(&self, bounds: Rect) {
        let dpi = self.ui.dpi();
        let header = dip(HEADER_HEIGHT).to_px(dpi).value();
        let inset = dip(INSET).to_px(dpi).value();
        let button_width = dip(CLEAR_WIDTH).to_px(dpi).value();
        let button_height = dip(BUTTON_HEIGHT).to_px(dpi).value();
        let button_top = bounds.top + (header - button_height) / 2;
        self.ui.apply_moves(&[
            (
                self.header.id(),
                Rect::new(
                    bounds.left + inset,
                    bounds.top,
                    bounds.right - button_width,
                    bounds.top + header,
                ),
            ),
            (
                self.clear.id(),
                Rect::new(
                    bounds.right - button_width,
                    button_top,
                    bounds.right - inset,
                    button_top + button_height,
                ),
            ),
            (
                self.list.id(),
                Rect::new(
                    bounds.left,
                    bounds.top + header,
                    bounds.right,
                    bounds.bottom,
                ),
            ),
        ]);
    }

    /// Shows or hides the whole view (its band and list).
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.header.id(), visible);
        self.ui.set_visible(self.clear.id(), visible);
        self.ui.set_visible(self.list.id(), visible);
    }

    /// Refreshes the list from the library. `rebuild` is set when the library
    /// (and so the history) or the search changed; the list is also rebuilt on
    /// the first sync and whenever the playing track changes, because a play's
    /// status cell reads "Playing" only for the current track.
    pub fn sync(
        &mut self,
        library: &dyn LibraryDataSource,
        search: &SearchEngine,
        playing_id: Option<u64>,
        rebuild: bool,
    ) {
        if !self.built.get()
            || rebuild
            || self.playing.get() != playing_id
            || self.applied_search.get() != search.revision()
        {
            self.rebuild(library, search, playing_id);
            self.built.set(true);
            self.applied_search.set(search.revision());
        }
    }

    /// Rebuilds the model from the library's history, keeping only plays whose
    /// track matches the search, and updates the band.
    fn rebuild(
        &mut self,
        library: &dyn LibraryDataSource,
        search: &SearchEngine,
        playing_id: Option<u64>,
    ) {
        let entries = library.history();
        let now = unix_now();
        let rows = build_rows(entries, now, playing_id, search);
        let kept = entries
            .iter()
            .filter(|entry| filter::track_kept(Some(search), entry.track_id))
            .count();
        self.header.set_text(&format!("{kept} plays"));
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
}

/// Flattens the shared grouping into pre-formatted rows, keeping only the
/// plays whose track matches `search`.
fn build_rows(
    entries: &[HistoryEntry],
    now: i64,
    playing_id: Option<u64>,
    search: &SearchEngine,
) -> Vec<HistoryRow> {
    history::build_rows_filtered(entries, now, |entry| {
        filter::track_kept(Some(search), entry.track_id)
    })
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
        let rows = build_rows(&entries, now, Some(20), &SearchEngine::new());

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
