//! Rendering of the History view's grouped table.
//!
//! Rows are laid out by hand (rather than through `egui_extras`) because the
//! list is interspersed with full-width day headers, which the shared track
//! table's column model has no room for. The look still matches the other
//! list views: left-aligned cells and alternating row backgrounds, with
//! double-click / context-menu playback (#24).

use std::time::Duration;

use eframe::egui;

use super::grouping::{self, Row};
use crate::library_api::{HistoryEntry, format_minutes_ago};
use crate::views::track_table::columns;

/// A playback action requested from a history row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryAction {
    /// The row was double-clicked (or Play chosen): play the recorded track.
    Play(u64),
    /// The row's remove button (or Remove chosen) was clicked.
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
    let stripes = entry_stripes(&rows);
    header(ui);

    let mut action = None;
    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .auto_shrink([false, false])
        .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
            for i in range {
                match &rows[i] {
                    Row::Day(label) => day_header(ui, label),
                    Row::Entry(entry) => {
                        if let Some(requested) = entry_row(ui, entry, now, playing_id, stripes[i]) {
                            action = Some(requested);
                        }
                    }
                }
            }
        });
    action
}

/// Whether each row should get the darker stripe, alternating between entry
/// rows only (day headers never stripe, and don't disturb the alternation,
/// matching the shared track table's `striped` look).
fn entry_stripes(rows: &[Row<'_>]) -> Vec<bool> {
    let mut ordinal = 0;
    rows.iter()
        .map(|row| match row {
            Row::Day(_) => false,
            Row::Entry(_) => {
                let stripe = ordinal % 2 == 1;
                ordinal += 1;
                stripe
            }
        })
        .collect()
}

/// Width available to the title cell, measured from the full row width so
/// the header and every entry row line up.
fn title_width(ui: &egui::Ui) -> f32 {
    let gaps = ui.spacing().item_spacing.x * 5.0;
    (ui.available_width() - FIXED_WIDTH - gaps).max(80.0)
}

/// A fixed-size, left-aligned text cell. The exact width is allocated (so
/// the columns stay aligned even for short values), then the text is drawn
/// flush left inside it.
fn text_cell(ui: &mut egui::Ui, width: f32, text: egui::RichText) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), egui::Sense::hover());
    let mut cell = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    cell.add(egui::Label::new(text).truncate().selectable(false));
}

/// A left-aligned, strong header cell, matching the entry cells' alignment.
fn header_cell(ui: &mut egui::Ui, width: f32, name: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), egui::Sense::hover());
    let mut cell = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    cell.add(egui::Label::new(egui::RichText::new(name).strong()).selectable(false));
}

fn header(ui: &mut egui::Ui) {
    let title = title_width(ui);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::hover(),
    );
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    header_cell(&mut row, TIME_WIDTH, "Time");
    header_cell(&mut row, title, "Title");
    header_cell(&mut row, ARTIST_WIDTH, "Artist");
    header_cell(&mut row, LISTENED_WIDTH, "Listened");
    header_cell(&mut row, COMPLETED_WIDTH, "Completed");
    header_cell(&mut row, REMOVE_WIDTH, "");
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
    striped: bool,
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

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );
    if striped {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().faint_bg_color);
    }

    let mut action = None;
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let title_w = title_width(&row);
    text_cell(
        &mut row,
        TIME_WIDTH,
        egui::RichText::new(format_minutes_ago(minutes)).weak(),
    );
    text_cell(&mut row, title_w, tinted(title, is_playing));
    text_cell(&mut row, ARTIST_WIDTH, tinted(artist, is_playing));
    text_cell(
        &mut row,
        LISTENED_WIDTH,
        egui::RichText::new(listened).weak(),
    );
    text_cell(&mut row, COMPLETED_WIDTH, completed_text(entry.completed));

    let remove = row.add_sized([REMOVE_WIDTH, ROW_HEIGHT], egui::Button::new("✕").small());
    if remove.clicked() {
        action = Some(HistoryAction::Remove(entry.id));
    }
    remove.on_hover_text("Remove this entry");

    if action.is_none() {
        if response.double_clicked() {
            action = Some(HistoryAction::Play(entry.track_id));
        }
        response.context_menu(|ui| {
            if ui.button("Play").clicked() {
                action = Some(HistoryAction::Play(entry.track_id));
                ui.close();
            }
            if ui.button("Remove").clicked() {
                action = Some(HistoryAction::Remove(entry.id));
                ui.close();
            }
        });
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
