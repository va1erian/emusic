//! Bottom status bar: track count, total library duration, a short status
//! text, and (when enabled in Settings) the optional visualizer strip.
//!
//! Render-only (#104): the part texts live in
//! [`emusic_ui::panels::status_bar::StatusBar`]; this module draws them and
//! maps its buttons to commands.

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    state
        .status_bar
        .sync(state.search_result_count, library, player);
    // Copy the parts out first: the buttons below need `&mut state`, and the
    // model is borrowed through `state`.
    let result_count = state.status_bar.result_count().to_owned();
    let total_duration = state.status_bar.total_duration().to_owned();
    let status = state.status_bar.status().to_owned();
    let shuffle_scope = state.status_bar.shuffle_scope().map(str::to_owned);
    let status_message = state.status_bar.status_message().map(str::to_owned);
    let scan = state.status_bar.scan().map(str::to_owned);
    let scanning = state.status_bar.is_scanning();
    let auto_tag = state.status_bar.auto_tag().map(str::to_owned);

    egui::Panel::bottom("status_bar")
        .exact_size(28.0)
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(&result_count);
                ui.separator();
                ui.label(&total_duration);
                ui.separator();
                ui.label(&status);
                if let Some(scope) = &shuffle_scope {
                    ui.separator();
                    ui.label(format!("Shuffling: {scope}"));
                    if ui.small_button("Stop").clicked() {
                        state.push(Command::PlayerToggleShuffle);
                    }
                }
                if let Some(message) = &status_message {
                    ui.separator();
                    ui.colored_label(ui.visuals().warn_fg_color, message);
                }
                if let Some(text) = &scan {
                    ui.separator();
                    ui.label(text);
                    if scanning && ui.small_button("Cancel").clicked() {
                        state.push(Command::LibraryCancelScan);
                    }
                }
                if let Some(text) = &auto_tag {
                    ui.separator();
                    ui.label(text);
                    if ui.small_button("Cancel").clicked() {
                        state.push(Command::CancelAutoTag);
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if state.visualizer_enabled {
                        super::visualizer::show(ui, state, player);
                    }
                });
            });
        });
}
