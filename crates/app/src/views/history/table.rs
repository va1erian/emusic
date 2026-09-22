//! Rendering of the History view's grouped, virtualized table.
//!
//! Rows are laid out by hand (rather than through `egui_extras`) because the
//! list is interspersed with full-width day headers, which the shared track
//! table's column model has no room for. The track table's duration helper is
//! reused for a consistent "Listened" column (#15, #24).

use std::time::Duration;

use eframe::egui;

use super::grouping::{self, Row};
use crate::library_api::{HistoryEntry, format_minutes_ago};
use crate::views::track_table::columns;

/// A playback action requested from a history row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryAction {
    /// The row was double-clicked: play the track this entry recorded.
    Play(u64),
    /// The row's remove button was clicked.
    Remove(i64),
}

const ROW_HEIGHT: f32 = 20.0;
const TIME_WIDTH: f32 = 92.0;
const ARTIST_WIDTH: f32 = 160.0;
const LISTENED_WIDTH: f32 = 64.0;
const COMPLETED_WIDTH: f32 = 84.0;
const REMOVE_WIDTH: f32 = 30.0;
/// Fixed columns plus the five inter-cell gaps; the title takes what's left.
const FIXED_WIDTH: f32 =
    TIME_WIDTH + ARTIST_WIDTH + LISTENED_WIDTH + COMPLETED_WIDTH + REMOVE_WIDTH;

/// Draws the history list and returns any action the user requested.
///
/// `now` is the current Unix time, passed in so grouping and relative ages
/// are testable without reading the clock; `playing_id` highlights the entry
/// for the currently playing track, if any.
pub fn show(
    ui: &mut egui::Ui,
    id_salt: &str,
    entries: &[HistoryEntry],
    now: i64,
    playing_id: Option<u64>,
) -> Option<HistoryAction> {
    let rows = grouping::build_rows(entries, now);
    header(ui);

    let mut action = None;
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .auto_shrink([false, false])
        .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
            for row in &rows[range] {
                match row {
                    Row::Day(label) => day_header(ui, label),
                    Row::Entry(entry) => {
                        if let Some(requested) = entry_row(ui, entry, now, playing_id) {
                            action = Some(requested);
                        }
                    }
                }
            }
        });
    action
}

/// Width available to the title cell, measured from the full row width so
/// the header and every entry row line up.
fn title_width(ui: &egui::Ui) -> f32 {
    let gaps = ui.spacing().item_spacing.x * 5.0;
    (ui.available_width() - FIXED_WIDTH - gaps).max(80.0)
}

/// A fixed-size text cell. `add_sized` reserves the full width even when the
/// text is shorter (which keeps the columns aligned); egui centres the text
/// within the cell.
fn text_cell(ui: &mut egui::Ui, width: f32, text: egui::RichText) -> egui::Response {
    ui.add_sized(
        [width, ROW_HEIGHT],
        egui::Label::new(text).truncate().selectable(false),
    )
}

fn header(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let title = title_width(ui);
        for (width, name) in [
            (TIME_WIDTH, "Time"),
            (title, "Title"),
            (ARTIST_WIDTH, "Artist"),
            (LISTENED_WIDTH, "Listened"),
            (COMPLETED_WIDTH, "Completed"),
            (REMOVE_WIDTH, ""),
        ] {
            ui.add_sized(
                [width, ROW_HEIGHT],
                egui::Label::new(egui::RichText::new(name).strong()).selectable(false),
            );
        }
    });
}

/// A full-width, left-aligned header row introducing one day's entries.
fn day_header(ui: &mut egui::Ui, day: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(ROW_HEIGHT);
        ui.add(egui::Label::new(egui::RichText::new(day).strong()).selectable(false));
    });
}

fn entry_row(
    ui: &mut egui::Ui,
    entry: &HistoryEntry,
    now: i64,
    playing_id: Option<u64>,
) -> Option<HistoryAction> {
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
    let minutes = ((now - entry.played_at).max(0) / 60) as u32;
    let listened = columns::format_duration(Duration::from_millis(u64::from(entry.played_ms)));
    let is_playing = playing_id == Some(entry.track_id);

    let mut action = None;
    let response = ui
        .horizontal(|ui| {
            let title_w = title_width(ui);
            text_cell(
                ui,
                TIME_WIDTH,
                egui::RichText::new(format_minutes_ago(minutes)).weak(),
            );
            text_cell(ui, title_w, tinted(title, is_playing));
            text_cell(ui, ARTIST_WIDTH, tinted(artist, is_playing));
            text_cell(ui, LISTENED_WIDTH, egui::RichText::new(listened).weak());
            text_cell(ui, COMPLETED_WIDTH, completed_text(entry.completed));

            let remove = ui.add_sized([REMOVE_WIDTH, ROW_HEIGHT], egui::Button::new("✕").small());
            if remove.clicked() {
                action = Some(HistoryAction::Remove(entry.id));
            }
            remove.on_hover_text("Remove this entry");
        })
        .response;

    if action.is_none() {
        let response = response.interact(egui::Sense::click());
        if response.double_clicked() {
            action = Some(HistoryAction::Play(entry.track_id));
        }
    }
    action
}

/// Text tint: the accent colour for the playing track, plain otherwise.
fn tinted(text: &str, is_playing: bool) -> egui::RichText {
    if is_playing {
        egui::RichText::new(text).color(crate::theme::current_accent())
    } else {
        egui::RichText::new(text)
    }
}

fn completed_text(completed: bool) -> egui::RichText {
    if completed {
        egui::RichText::new("Completed")
    } else {
        egui::RichText::new("Skipped").weak()
    }
}
