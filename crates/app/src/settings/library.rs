//! "Library" settings page (#19): the configured root folders, a native
//! folder picker to add more, per-folder Remove, and "Rescan now".
//!
//! Changes are queued as [`Command`]s; the shell persists them to the config
//! and applies them to the library backend, which triggers an incremental
//! scan (and purges a removed folder's tracks).

use eframe::egui;

use crate::state::{AppState, Command};

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label("Music folders");
    ui.label(
        egui::RichText::new(
            "Folders scanned for music. Adding or removing one rescans in the \
             background; progress appears in the status bar.",
        )
        .weak(),
    );
    ui.add_space(8.0);

    if state.library_folders.is_empty() {
        ui.label(egui::RichText::new("No folders yet.").weak().italics());
    } else {
        folder_list(ui, state);
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Add folder...").clicked() {
            crate::settings::folder_picker::request();
        }
        if ui.button("Rescan now").clicked() {
            state.push(Command::LibraryRescan);
        }
    });
}

/// One row per configured folder, each with a Remove button. Removals are
/// queued as commands so the shell updates the backend and config the same
/// way it does additions.
fn folder_list(ui: &mut egui::Ui, state: &mut AppState) {
    let mut remove = None;
    for path in &state.library_folders {
        ui.horizontal(|ui| {
            if ui.button("Remove").clicked() {
                remove = Some(path.clone());
            }
            ui.label(path.display().to_string());
        });
    }
    if let Some(path) = remove {
        state.push(Command::LibraryRemoveFolder(path));
    }
}
