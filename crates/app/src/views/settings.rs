//! "Settings" view placeholder: theme toggle lives here for now. Real
//! settings (library paths, tracker playback options, ...) are later
//! issues (#8, #19).

use eframe::egui;

use crate::state::{AppState, Command};

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label("Appearance");
    ui.separator();
    if ui.button("Toggle dark / light theme").clicked() {
        state.push(Command::ToggleTheme);
    }
}
