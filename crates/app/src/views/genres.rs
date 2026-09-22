//! "Genres" view placeholder: a flat list with per-genre track counts.
//! Each row's context menu can start a scoped shuffle of that genre (#57).

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
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
            let response = ui
                .horizontal(|ui| {
                    let name = ui.add_sized(
                        [200.0, 16.0],
                        egui::Label::new(genre).sense(egui::Sense::click()),
                    );
                    ui.label(egui::RichText::new(format!("{count} tracks")).weak());
                    name
                })
                .inner;
            response.context_menu(|ui| {
                if ui.button("Shuffle play").clicked() {
                    state.push(crate::shuffle::genre(library, genre));
                    ui.close();
                }
            });
        }
    });
}
