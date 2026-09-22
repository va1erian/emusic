//! "Artists" view placeholder: a flat sorted list with per-artist counts.
//! Each row's context menu can start a scoped shuffle of that artist (#57).

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    let mut artists: Vec<_> = library.artists().iter().collect();
    artists.sort_by(|a, b| a.name.cmp(&b.name));

    ui.label(egui::RichText::new(format!("{} artists", artists.len())).weak());
    ui.separator();

    egui::ScrollArea::vertical().show_rows(ui, 18.0, artists.len(), |ui, range| {
        for artist in &artists[range] {
            let response = ui
                .horizontal(|ui| {
                    let name = ui.add_sized(
                        [260.0, 16.0],
                        egui::Label::new(&artist.name)
                            .truncate()
                            .sense(egui::Sense::click()),
                    );
                    ui.label(egui::RichText::new(format!("{} albums", artist.album_count)).weak());
                    ui.label(egui::RichText::new(format!("{} tracks", artist.track_count)).weak());
                    name
                })
                .inner;
            response.context_menu(|ui| {
                if ui.button("Shuffle play").clicked() {
                    state.push(crate::shuffle::artist(library, &artist.name));
                    ui.close();
                }
            });
        }
    });
}
