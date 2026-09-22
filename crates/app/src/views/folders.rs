//! "Folders" view placeholder: a flat list of source folders. A real
//! collapsible folder tree is issue #18.

use eframe::egui;

use crate::library_api::LibraryDataSource;

pub fn show(ui: &mut egui::Ui, library: &dyn LibraryDataSource) {
    let folders = library.folders();
    ui.label(egui::RichText::new(format!("{} folders", folders.len())).weak());
    ui.separator();

    egui::ScrollArea::vertical().show_rows(ui, 18.0, folders.len(), |ui, range| {
        for folder in &folders[range] {
            ui.horizontal(|ui| {
                ui.add_sized([460.0, 16.0], egui::Label::new(&folder.path).truncate());
                ui.label(egui::RichText::new(format!("{} tracks", folder.track_count)).weak());
            });
        }
    });
}
