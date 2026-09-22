//! "Albums" view placeholder: a simple grid of album tiles. A real cover-art
//! grid is issue #17.

use eframe::egui;

use crate::library_api::LibraryDataSource;

const TILE_SIZE: f32 = 140.0;

pub fn show(ui: &mut egui::Ui, library: &dyn LibraryDataSource) {
    let albums = library.albums();
    ui.label(egui::RichText::new(format!("{} albums", albums.len())).weak());
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for album in albums {
                ui.allocate_ui(egui::vec2(TILE_SIZE, TILE_SIZE + 40.0), |ui| {
                    ui.vertical(|ui| {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(TILE_SIZE, TILE_SIZE),
                            egui::Sense::hover(),
                        );
                        ui.painter()
                            .rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "💿",
                            egui::FontId::proportional(36.0),
                            ui.visuals().weak_text_color(),
                        );
                        ui.add_sized([TILE_SIZE, 16.0], egui::Label::new(&album.name).truncate());
                        ui.add_sized(
                            [TILE_SIZE, 14.0],
                            egui::Label::new(egui::RichText::new(&album.artist).weak()).truncate(),
                        );
                    });
                });
            }
        });
    });
}
