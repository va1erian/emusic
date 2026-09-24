//! Clickable artist/album links shared by the now-playing surfaces: clicking
//! one records a navigation message.

use eframe::egui;

use emusic_ui::views::now_playing::NowPlayingMsg;

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
pub(super) fn artist(ui: &mut egui::Ui, name: &str, size: f32, messages: &mut Vec<NowPlayingMsg>) {
    if name.trim().is_empty() {
        return;
    }
    if link(ui, egui::RichText::new(name).size(size), "Go to artist") {
        messages.push(NowPlayingMsg::GoToArtist(name.to_string()));
    }
}

/// A clickable album name; clicking jumps to the Albums view with the album
/// selected. `artist` disambiguates same-named albums.
pub(super) fn album(
    ui: &mut egui::Ui,
    name: &str,
    artist: &str,
    messages: &mut Vec<NowPlayingMsg>,
) {
    if name.trim().is_empty() {
        return;
    }
    if link(ui, egui::RichText::new(name), "Go to album") {
        messages.push(NowPlayingMsg::GoToAlbum {
            name: name.to_string(),
            artist: artist.to_string(),
        });
    }
}
