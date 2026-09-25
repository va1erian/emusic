//! File -> Database info dialog (#193): library counts, the database file's
//! location and size, when the last scan finished, and a button that
//! rescans every library folder. The field contents come from
//! [`emusic_ui::panels::database_info`].

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::state::{AppState, Command};
use emusic_ui::panels::database_info::fields;

/// Shows the modal while `state.database_info_open` is set and clears the flag
/// when the user closes it (Close button, Escape or a backdrop click).
pub fn show(ctx: &egui::Context, state: &mut AppState, library: &dyn LibraryDataSource) {
    if !state.database_info_open {
        return;
    }
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new("database_info")).show(ctx, |ui| {
        ui.set_width(460.0);
        ui.heading("Database info");
        ui.add_space(6.0);

        egui::Grid::new("database_info_fields")
            .num_columns(2)
            .spacing([16.0, 5.0])
            .show(ui, |ui| {
                for (label, value) in fields(library) {
                    field(ui, label, &value);
                }
            });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let scanning = library.is_scanning();
            let rescan = ui
                .add_enabled(!scanning, egui::Button::new("Rescan everything"))
                .on_hover_text("Rescan every library folder")
                .on_disabled_hover_text("A scan is already running");
            if rescan.clicked() {
                state.push(Command::LibraryRescan);
            }
            if scanning {
                ui.weak("Scanning...");
            }
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    });

    if close || modal.should_close() {
        state.database_info_open = false;
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).strong());
    ui.add(egui::Label::new(value).selectable(true).wrap());
    ui.end_row();
}
