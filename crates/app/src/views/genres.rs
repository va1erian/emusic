//! "Genres" view placeholder: a flat list with per-genre track counts.

use eframe::egui;

use crate::library_api::LibraryDataSource;

pub fn show(ui: &mut egui::Ui, library: &dyn LibraryDataSource) {
    let genres = library.genres();
    ui.label(egui::RichText::new(format!("{} genres", genres.len())).weak());
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for genre in genres {
            let count = library
                .tracks()
                .iter()
                .filter(|t| &t.genre == genre)
                .count();
            ui.horizontal(|ui| {
                ui.add_sized([200.0, 16.0], egui::Label::new(genre));
                ui.label(egui::RichText::new(format!("{count} tracks")).weak());
            });
        }
    });
}
