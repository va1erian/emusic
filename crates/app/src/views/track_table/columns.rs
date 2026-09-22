//! Column definitions for the track table: identity, header label, sizing
//! and how to render/derive each column's text for a given track.

use eframe::egui;

use crate::library_api::{TrackInfo, format_minutes_ago};

/// Identifies one column. The `#` (row position) column is intentionally
/// excluded here: it is not backed by track data and is never sortable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnId {
    Title,
    Artist,
    Album,
    Year,
    Genre,
    Time,
    Format,
    Plays,
    LastPlayed,
    File,
}

/// Static description of one column: label, default/min width and whether
/// header clicks should sort by it.
pub struct ColumnSpec {
    pub id: ColumnId,
    pub label: &'static str,
    pub initial_width: f32,
    pub min_width: f32,
}

/// All data columns, left to right (the `#` column is drawn separately by
/// the table widget itself).
// `Title` is deliberately excluded from this list: the table gives it the
// remaining space (`egui_extras::Column::remainder`) instead of a fixed
// initial width, since it's the column users most want to read in full and
// egui_extras tables can't scroll horizontally to reveal clipped columns.
pub const COLUMNS: &[ColumnSpec] = &[
    ColumnSpec {
        id: ColumnId::Artist,
        label: "Artist",
        initial_width: 130.0,
        min_width: 70.0,
    },
    ColumnSpec {
        id: ColumnId::Album,
        label: "Album",
        initial_width: 130.0,
        min_width: 70.0,
    },
    ColumnSpec {
        id: ColumnId::Year,
        label: "Year",
        initial_width: 46.0,
        min_width: 40.0,
    },
    ColumnSpec {
        id: ColumnId::Genre,
        label: "Genre",
        initial_width: 84.0,
        min_width: 56.0,
    },
    ColumnSpec {
        id: ColumnId::Time,
        label: "Time",
        initial_width: 50.0,
        min_width: 44.0,
    },
    ColumnSpec {
        id: ColumnId::Format,
        label: "Format",
        initial_width: 52.0,
        min_width: 44.0,
    },
    ColumnSpec {
        id: ColumnId::Plays,
        label: "Plays",
        initial_width: 46.0,
        min_width: 40.0,
    },
    ColumnSpec {
        id: ColumnId::LastPlayed,
        label: "Last played",
        initial_width: 82.0,
        min_width: 64.0,
    },
    ColumnSpec {
        id: ColumnId::File,
        label: "File",
        initial_width: 140.0,
        min_width: 90.0,
    },
];

/// Title column sizing, handled separately from [`COLUMNS`] since it grows
/// to fill leftover space rather than taking a fixed initial width.
pub const TITLE_MIN_WIDTH: f32 = 120.0;

/// Placeholder shown for an empty/unknown tag value, matching the flat list
/// view's existing convention.
pub fn title_text(track: &TrackInfo) -> &str {
    if track.title.is_empty() {
        "(unknown title)"
    } else {
        &track.title
    }
}

pub fn artist_text(track: &TrackInfo) -> &str {
    if track.artist.is_empty() {
        "(unknown artist)"
    } else {
        &track.artist
    }
}

/// Renders a column's cell content (left-aligned, MusicBee-style) for
/// `track` into `ui`. `is_playing` tints the text with the accent colour so
/// the currently playing row stands out at a glance.
pub fn show_cell(ui: &mut egui::Ui, id: ColumnId, track: &TrackInfo, is_playing: bool) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        let tint = |text: &str| {
            let rich = egui::RichText::new(text);
            if is_playing {
                rich.color(crate::theme::ACCENT).strong()
            } else {
                rich
            }
        };
        let weak_tint = |text: String| {
            if is_playing {
                egui::RichText::new(text).color(crate::theme::ACCENT)
            } else {
                egui::RichText::new(text).weak()
            }
        };

        match id {
            ColumnId::Title => {
                let text = if is_playing {
                    format!("▶ {}", title_text(track))
                } else {
                    title_text(track).to_string()
                };
                ui.add(egui::Label::new(tint(&text)).truncate());
            }
            ColumnId::Artist => {
                ui.add(egui::Label::new(tint(artist_text(track))).truncate());
            }
            ColumnId::Album => {
                ui.add(egui::Label::new(tint(&track.album)).truncate());
            }
            ColumnId::Year => {
                let text = track.year.map(|y| y.to_string()).unwrap_or_default();
                ui.add(egui::Label::new(weak_tint(text)).truncate());
            }
            ColumnId::Genre => {
                ui.add(egui::Label::new(tint(&track.genre)).truncate());
            }
            ColumnId::Time => {
                ui.add(egui::Label::new(tint(&format_duration(track.duration))).truncate());
            }
            ColumnId::Format => {
                ui.add(egui::Label::new(weak_tint(track.format.clone())).truncate());
            }
            ColumnId::Plays => {
                ui.add(egui::Label::new(tint(&track.play_count.to_string())).truncate());
            }
            ColumnId::LastPlayed => {
                let text = track
                    .last_played_minutes_ago
                    .map(format_minutes_ago)
                    .unwrap_or_default();
                ui.add(egui::Label::new(weak_tint(text)).truncate());
            }
            ColumnId::File => {
                ui.add(
                    egui::Label::new(weak_tint(track.path.clone()))
                        .truncate()
                        .selectable(false),
                );
            }
        }
    });
}

pub fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
