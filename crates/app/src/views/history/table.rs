//! Rendering of the History view's grouped table.
//!
//! Built on the same `egui_extras` table setup as the shared track table
//! (striped rows, clipped columns, click selection, arrow-key/Enter
//! navigation, a star toggle and a context menu), so History behaves like
//! every other list view. Day headers are ordinary rows that carry the day's
//! label in the first column, which the track table's model has no room for
//! (#24).

use std::collections::HashSet;
use std::time::Duration;

use eframe::egui;
use egui_extras::{Column, TableBuilder, TableRow};

use super::grouping::{self, Row};
use crate::library_api::{HistoryEntry, LibraryDataSource, TrackInfo, format_minutes_ago};
use crate::views::track_table::columns;
use crate::views::track_table::{ClickModifiers, SelectionState};

/// An action requested from a history row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryAction {
    /// The row was double-clicked (or Play / Enter chosen): play the track.
    Play(u64),
    PlayNext(u64),
    AddToQueue(u64),
    ToggleStar(u64),
    EditTags(u64),
    /// The row's remove button (or Remove chosen) was clicked.
    Remove(i64),
}

const ROW_HEIGHT: f32 = 20.0;
const HEADER_HEIGHT: f32 = 22.0;
const TIME_WIDTH: f32 = 96.0;
const STAR_WIDTH: f32 = 24.0;
const TITLE_MIN_WIDTH: f32 = 120.0;
const ARTIST_WIDTH: f32 = 160.0;
const LISTENED_WIDTH: f32 = 64.0;
const COMPLETED_WIDTH: f32 = 84.0;
const REMOVE_WIDTH: f32 = 30.0;
const HEADERS: [&str; 7] = ["Time", "", "Title", "Artist", "Listened", "Completed", ""];

/// Draws the history list and returns any action the user requested.
///
/// `now` is the current Unix time, passed in so grouping and relative ages
/// are testable without reading the clock; `playing_id` highlights the entry
/// for the currently playing track, if any.
pub fn show(
    ui: &mut egui::Ui,
    id_salt: &str,
    selection: &mut SelectionState,
    entries: &[HistoryEntry],
    library: &dyn LibraryDataSource,
    now: i64,
    playing_id: Option<u64>,
) -> Option<HistoryAction> {
    let rows = grouping::build_rows(entries, now);
    // Entries in display order (day headers skipped): selection and keyboard
    // focus index into this, keyed by the play's own id.
    let positions = entry_positions(&rows);
    let order: Vec<&HistoryEntry> = rows
        .iter()
        .filter_map(|row| match row {
            Row::Entry(entry) => Some(*entry),
            Row::Day(_) => None,
        })
        .collect();
    let order_ids: Vec<u64> = order.iter().map(|entry| entry.id as u64).collect();
    selection.retain_existing(&order_ids.iter().copied().collect::<HashSet<u64>>());

    let mut action = keyboard(ui, selection, &order, &order_ids);

    let available_height = ui.available_height();
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), available_height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            TableBuilder::new(ui)
                .id_salt(id_salt)
                .striped(true)
                .sense(egui::Sense::click())
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .min_scrolled_height(0.0)
                .max_scroll_height(available_height)
                .column(Column::exact(TIME_WIDTH).clip(true))
                .column(Column::exact(STAR_WIDTH))
                .column(Column::remainder().at_least(TITLE_MIN_WIDTH).clip(true))
                .column(
                    Column::initial(ARTIST_WIDTH)
                        .at_least(70.0)
                        .resizable(true)
                        .clip(true),
                )
                .column(Column::exact(LISTENED_WIDTH).clip(true))
                .column(Column::exact(COMPLETED_WIDTH).clip(true))
                .column(Column::exact(REMOVE_WIDTH))
                .header(HEADER_HEIGHT, |mut header| {
                    for name in HEADERS {
                        header.col(|ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::new(name).strong())
                                    .selectable(false),
                            );
                        });
                    }
                })
                .body(|body| {
                    body.rows(ROW_HEIGHT, rows.len(), |mut row| {
                        let i = row.index();
                        match &rows[i] {
                            Row::Day(label) => day_row(&mut row, label),
                            Row::Entry(entry) => {
                                let ctx = EntryCtx {
                                    entry,
                                    track: library.tracks().iter().find(|t| t.id == entry.track_id),
                                    now,
                                    is_playing: playing_id == Some(entry.track_id),
                                    pos: positions[i],
                                };
                                if let Some(requested) =
                                    entry_row(&mut row, &ctx, selection, &order_ids)
                                {
                                    action = Some(requested);
                                }
                            }
                        }
                    });
                });
        },
    );
    action
}

/// For each row, its position among entry rows (day headers get a
/// placeholder), used to index the selection order.
fn entry_positions(rows: &[Row<'_>]) -> Vec<usize> {
    let mut next = 0;
    rows.iter()
        .map(|row| match row {
            Row::Day(_) => 0,
            Row::Entry(_) => {
                next += 1;
                next - 1
            }
        })
        .collect()
}

/// Arrow-key navigation and Enter-to-play, only when no text field holds
/// keyboard focus (same rule as the shared track table).
fn keyboard(
    ui: &egui::Ui,
    selection: &mut SelectionState,
    order: &[&HistoryEntry],
    order_ids: &[u64],
) -> Option<HistoryAction> {
    if ui.memory(|m| m.focused().is_some()) {
        return None;
    }
    let (delta, enter) = ui.input(|i| {
        let delta = if i.key_pressed(egui::Key::ArrowDown) {
            1
        } else if i.key_pressed(egui::Key::ArrowUp) {
            -1
        } else {
            0
        };
        (delta, i.key_pressed(egui::Key::Enter))
    });
    if delta != 0 {
        selection.move_focus(order_ids, delta, order.len());
    }
    let focused = selection.focus.and_then(|focus| order.get(focus))?;
    enter.then_some(HistoryAction::Play(focused.track_id))
}

/// A day header: the label in the first column, the rest empty.
fn day_row(row: &mut TableRow<'_, '_>, day: &str) {
    row.col(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(day).strong()).selectable(false));
    });
    for _ in 1..HEADERS.len() {
        row.col(|_ui| {});
    }
}

/// Everything one entry row needs besides the shared selection state.
struct EntryCtx<'a> {
    entry: &'a HistoryEntry,
    /// The library track the entry refers to, if it still exists.
    track: Option<&'a TrackInfo>,
    now: i64,
    is_playing: bool,
    /// Position among entry rows, for selection ranges.
    pos: usize,
}

fn entry_row(
    row: &mut TableRow<'_, '_>,
    ctx: &EntryCtx<'_>,
    selection: &mut SelectionState,
    order_ids: &[u64],
) -> Option<HistoryAction> {
    let entry = ctx.entry;
    let title = if entry.title.is_empty() {
        "(unknown title)"
    } else {
        entry.title.as_str()
    };
    let artist = if entry.artist.is_empty() {
        "(unknown artist)"
    } else {
        entry.artist.as_str()
    };
    let minutes = ((ctx.now - entry.played_at).max(0) / 60) as u32;
    let listened = columns::format_duration(Duration::from_millis(u64::from(entry.played_ms)));
    let entry_key = entry.id as u64;
    row.set_selected(selection.is_selected(entry_key));

    let mut action = None;
    row.col(|ui| {
        text(ui, egui::RichText::new(format_minutes_ago(minutes)).weak());
    });
    let mut star_clicked = false;
    row.col(|ui| {
        if let Some(track) = ctx.track
            && columns::star_cell(ui, track)
        {
            star_clicked = true;
            action = Some(HistoryAction::ToggleStar(entry.track_id));
        }
    });
    row.col(|ui| {
        if ctx.is_playing {
            columns::playing_marker(ui);
        }
        text(ui, tinted(title, ctx.is_playing));
    });
    row.col(|ui| text(ui, tinted(artist, ctx.is_playing)));
    row.col(|ui| text(ui, egui::RichText::new(listened).weak()));
    row.col(|ui| {
        text(
            ui,
            status_text(entry.finished, entry.completed, ctx.is_playing),
        );
    });
    let mut remove_clicked = false;
    row.col(|ui| {
        let remove = ui
            .add(egui::Button::new("✕").small())
            .on_hover_text("Remove this entry");
        if remove.clicked() {
            remove_clicked = true;
            action = Some(HistoryAction::Remove(entry.id));
        }
    });

    // A click on the star or remove button is that cell's own action, not a
    // row selection or a double-click-to-play.
    let response = row.response();
    let plain_click = !star_clicked && !remove_clicked;
    if plain_click && response.clicked() {
        let modifiers = response.ctx.input(|i| ClickModifiers {
            ctrl: i.modifiers.command,
            shift: i.modifiers.shift,
        });
        selection.click(order_ids, ctx.pos, entry_key, modifiers);
    }
    if plain_click && response.double_clicked() {
        action = Some(HistoryAction::Play(entry.track_id));
    }
    response.context_menu(|ui| {
        if let Some(chosen) = context_menu(ui, entry, ctx.track) {
            action = Some(chosen);
        }
    });
    action
}

/// The row's right-click menu, mirroring the track table's actions plus
/// Remove. Star and Edit tags need the library track, so they are omitted
/// when it no longer exists.
fn context_menu(
    ui: &mut egui::Ui,
    entry: &HistoryEntry,
    track: Option<&TrackInfo>,
) -> Option<HistoryAction> {
    let mut action = None;
    let mut item = |ui: &mut egui::Ui, label: &str, chosen: HistoryAction| {
        if ui.button(label).clicked() {
            action = Some(chosen);
            ui.close();
        }
    };
    let id = entry.track_id;
    item(ui, "Play", HistoryAction::Play(id));
    item(ui, "Play next", HistoryAction::PlayNext(id));
    item(ui, "Add to queue", HistoryAction::AddToQueue(id));
    if let Some(track) = track {
        ui.separator();
        let star = if track.starred { "Unstar" } else { "Star" };
        item(ui, star, HistoryAction::ToggleStar(id));
        ui.separator();
        item(ui, "Edit tags…", HistoryAction::EditTags(id));
    }
    ui.separator();
    item(ui, "Remove from history", HistoryAction::Remove(entry.id));
    action
}

fn text(ui: &mut egui::Ui, text: egui::RichText) {
    ui.add(egui::Label::new(text).truncate().selectable(false));
}

/// Text tint: the accent colour for the playing track, plain otherwise.
fn tinted(text: &str, is_playing: bool) -> egui::RichText {
    if is_playing {
        egui::RichText::new(text)
            .color(crate::theme::current_accent())
            .strong()
    } else {
        egui::RichText::new(text)
    }
}

/// Status text for a history row: a play that is still in progress reads
/// "Playing" while its track is the current one, otherwise it is classified
/// by the player's completion verdict once it has finished.
fn status_text(finished: bool, completed: bool, is_playing: bool) -> egui::RichText {
    let (label, weak) = status_label(finished, completed, is_playing);
    let text = egui::RichText::new(label);
    if weak { text.weak() } else { text }
}

/// The status label and whether it is styled as secondary (weak) text.
fn status_label(finished: bool, completed: bool, is_playing: bool) -> (&'static str, bool) {
    if !finished && is_playing {
        ("Playing", true)
    } else if completed {
        ("Completed", false)
    } else {
        ("Skipped", true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_label_prioritises_a_live_play_then_completion() {
        // A play in progress, on the current track, is "Playing" even though
        // it has no completion verdict yet.
        assert_eq!(status_label(false, false, true), ("Playing", true));
        // Unfinished but not the current track (e.g. after a restart): it is
        // treated as a skip rather than shown as playing forever.
        assert_eq!(status_label(false, false, false), ("Skipped", true));
        assert_eq!(status_label(true, true, true), ("Completed", false));
        assert_eq!(status_label(true, false, false), ("Skipped", true));
    }
}
