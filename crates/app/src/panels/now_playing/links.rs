//! Clickable artist/album links shared by the now-playing surfaces: clicking
//! one jumps to the matching view (and selects the album there).

use eframe::egui;

use crate::state::{AppState, Command};

/// Renders `text` as a hyperlink-styled, clickable label and returns whether
/// it was clicked. The caller decides what navigation the click means.
pub(super) fn link(ui: &mut egui::Ui, text: egui::RichText, hover: &str) -> bool {
    ui.add(
        egui::Label::new(text.color(ui.visuals().hyperlink_color))
            .sense(egui::Sense::click())
            .truncate(),
    )
    .on_hover_text(hover)
    .on_hover_cursor(egui::CursorIcon::PointingHand)
    .clicked()
}

/// A clickable artist name; clicking jumps to the Artists view.
pub(super) fn artist(ui: &mut egui::Ui, state: &mut AppState, name: &str, size: f32) {
    if name.trim().is_empty() {
        return;
    }
    if link(ui, egui::RichText::new(name).size(size), "Go to artist") {
        state.push(Command::GoToArtist(name.to_string()));
    }
}

/// A clickable album name; clicking jumps to the Albums view with the album
/// selected. `artist` disambiguates same-named albums.
pub(super) fn album(ui: &mut egui::Ui, state: &mut AppState, name: &str, artist: &str) {
    if name.trim().is_empty() {
        return;
    }
    if link(ui, egui::RichText::new(name), "Go to album") {
        state.push(Command::GoToAlbum {
            name: name.to_string(),
            artist: artist.to_string(),
        });
    }
}
