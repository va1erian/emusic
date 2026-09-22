//! "History" view: recently played tracks, most recent first.

use eframe::egui;

use crate::library_api::LibraryDataSource;

pub fn show(ui: &mut egui::Ui, library: &dyn LibraryDataSource) {
    let history = library.history();
    ui.label(egui::RichText::new(format!("{} plays", history.len())).weak());
    ui.separator();

    egui::ScrollArea::vertical().show_rows(ui, 18.0, history.len(), |ui, range| {
        for entry in &history[range] {
            ui.horizontal(|ui| {
                let title = if entry.track_title.is_empty() {
                    "(unknown title)"
                } else {
                    entry.track_title.as_str()
                };
                ui.add_sized([260.0, 16.0], egui::Label::new(title).truncate());
                ui.add_sized([180.0, 16.0], egui::Label::new(&entry.artist).truncate());
                ui.label(egui::RichText::new(&entry.played_at).weak());
            });
        }
    });
}
