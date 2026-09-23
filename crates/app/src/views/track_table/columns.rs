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

/// Side of the "now playing" triangle as a fraction of the row's text height,
/// so the marker stays proportional to the title at any DPI.
const PLAYING_MARKER_SCALE: f32 = 0.8;

/// Horizontal gap between the "now playing" marker and the title text.
const PLAYING_MARKER_GAP: f32 = 4.0;

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
                rich.color(crate::theme::current_accent()).strong()
            } else {
                rich
            }
        };
        let weak_tint = |text: String| {
            if is_playing {
                egui::RichText::new(text).color(crate::theme::current_accent())
            } else {
                egui::RichText::new(text).weak()
            }
        };

        match id {
            ColumnId::Title => {
                if is_playing {
                    playing_marker(ui);
                }
                ui.add(
                    egui::Label::new(tint(title_text(track)))
                        .truncate()
                        .selectable(false),
                );
            }
            ColumnId::Artist => {
                ui.add(
                    egui::Label::new(tint(artist_text(track)))
                        .truncate()
                        .selectable(false),
                );
            }
            ColumnId::Album => {
                ui.add(
                    egui::Label::new(tint(&track.album))
                        .truncate()
                        .selectable(false),
                );
            }
            ColumnId::Year => {
                let text = track.year.map(|y| y.to_string()).unwrap_or_default();
                ui.add(
                    egui::Label::new(weak_tint(text))
                        .truncate()
                        .selectable(false),
                );
            }
            ColumnId::Genre => {
                ui.add(
                    egui::Label::new(tint(&track.genre))
                        .truncate()
                        .selectable(false),
                );
            }
            ColumnId::Time => {
                ui.add(
                    egui::Label::new(tint(&format_duration(track.duration)))
                        .truncate()
                        .selectable(false),
                );
            }
            ColumnId::Format => {
                ui.add(
                    egui::Label::new(weak_tint(track.format.clone()))
                        .truncate()
                        .selectable(false),
                );
            }
            ColumnId::Plays => {
                ui.add(
                    egui::Label::new(tint(&track.play_count.to_string()))
                        .truncate()
                        .selectable(false),
                );
            }
            ColumnId::LastPlayed => {
                let text = track
                    .last_played_minutes_ago
                    .map(format_minutes_ago)
                    .unwrap_or_default();
                ui.add(
                    egui::Label::new(weak_tint(text))
                        .truncate()
                        .selectable(false),
                );
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

/// Renders the star toggle for `track` (filled when starred, outlined
/// otherwise) and returns whether it was clicked this frame (#131).
///
/// A painted vector icon (see [`crate::icons`]) rather than a `★` glyph, for
/// the same font-metric reasons as the playing marker; the cell's centred
/// layout keeps it centred on the row.
pub fn star_cell(ui: &mut egui::Ui, track: &TrackInfo) -> bool {
    let side = ui.text_style_height(&egui::TextStyle::Body) * PLAYING_MARKER_SCALE;
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::click());
    if track.starred {
        crate::icons::star(ui.painter(), rect, crate::theme::current_accent());
    } else {
        let color = ui.visuals().weak_text_color();
        crate::icons::star_outline(ui.painter(), rect, color);
    }
    let response = response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(if track.starred { "Unstar" } else { "Star" });
    response.clicked()
}

/// Reserves space for, and paints, the "now playing" triangle at the start of
/// a title cell. It is a painted vector shape (see [`crate::icons`]) rather
/// than a `▶` glyph, and the cell's centred layout vertically centres the
/// reserved box on the row, so the marker lines up with the row's text
/// instead of sitting high on the text baseline (#81).
pub(crate) fn playing_marker(ui: &mut egui::Ui) {
    let side = ui.text_style_height(&egui::TextStyle::Body) * PLAYING_MARKER_SCALE;
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::hover());
    crate::icons::play_in(ui.painter(), rect, crate::theme::current_accent());
    ui.add_space(PLAYING_MARKER_GAP);
}
