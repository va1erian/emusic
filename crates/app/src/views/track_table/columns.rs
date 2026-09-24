//! Cell painters for the track table's columns: how each column's text for
//! a given track is rendered into the UI.
//!
//! The column definitions themselves (identity, widths, text) live in
//! `emusic-ui` and are re-exported here so `columns::…` paths keep working.

use eframe::egui;

pub use emusic_ui::library_api::format_minutes_ago;
pub use emusic_ui::views::track_table::columns::{
    COLUMNS, ColumnId, ColumnSpec, TITLE_MIN_WIDTH, artist_text, format_duration, title_text,
};

use crate::library_api::TrackInfo;

/// Side of the "now playing" triangle as a fraction of the row's text height,
/// so the marker stays proportional to the title at any DPI.
const PLAYING_MARKER_SCALE: f32 = 0.8;

/// Side of the star toggle's click target as a fraction of the row's text
/// height (#193); larger than the playing marker so the star reads clearly.
const STAR_CELL_SCALE: f32 = 1.0;

/// Horizontal gap between the "now playing" marker and the title text.
const PLAYING_MARKER_GAP: f32 = 4.0;

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

/// Renders the star toggle for `track` (filled when starred, outlined
/// otherwise) and returns whether it was clicked this frame (#131).
///
/// A painted vector icon (see [`crate::icons`]) rather than a `★` glyph, for
/// the same font-metric reasons as the playing marker; the cell's centred
/// layout keeps it centred on the row.
pub fn star_cell(ui: &mut egui::Ui, track: &TrackInfo) -> bool {
    let side = ui.text_style_height(&egui::TextStyle::Body) * STAR_CELL_SCALE;
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
