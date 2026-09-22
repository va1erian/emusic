//! "Artists" view placeholder: a flat sorted list with per-artist counts.

use eframe::egui;

use crate::library_api::LibraryDataSource;

pub fn show(ui: &mut egui::Ui, library: &dyn LibraryDataSource) {
    let mut artists: Vec<_> = library.artists().iter().collect();
    artists.sort_by(|a, b| a.name.cmp(&b.name));

    ui.label(egui::RichText::new(format!("{} artists", artists.len())).weak());
    ui.separator();

    egui::ScrollArea::vertical().show_rows(ui, 18.0, artists.len(), |ui, range| {
        for artist in &artists[range] {
            ui.horizontal(|ui| {
                ui.add_sized([260.0, 16.0], egui::Label::new(&artist.name).truncate());
                ui.label(egui::RichText::new(format!("{} albums", artist.album_count)).weak());
                ui.label(egui::RichText::new(format!("{} tracks", artist.track_count)).weak());
            });
        }
    });
}
