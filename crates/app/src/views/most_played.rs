//! "Most played" view: tracks ordered by descending play count.

use eframe::egui;

use crate::library_api::LibraryDataSource;

pub fn show(ui: &mut egui::Ui, library: &dyn LibraryDataSource) {
    let tracks = library.most_played();
    ui.label(egui::RichText::new(format!("Top {} tracks", tracks.len())).weak());
    ui.separator();

    egui::ScrollArea::vertical().show_rows(ui, 18.0, tracks.len(), |ui, range| {
        for (i, track) in tracks.iter().enumerate().collect::<Vec<_>>()[range].iter() {
            ui.horizontal(|ui| {
                ui.add_sized([28.0, 16.0], egui::Label::new(format!("{}", i + 1)));
                let title = if track.title.is_empty() {
                    "(unknown title)"
                } else {
                    track.title.as_str()
                };
                ui.add_sized([260.0, 16.0], egui::Label::new(title).truncate());
                ui.add_sized([180.0, 16.0], egui::Label::new(&track.artist).truncate());
                ui.label(egui::RichText::new(format!("{} plays", track.play_count)).weak());
            });
        }
    });
}
