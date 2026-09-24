//! Bottom status bar: track count, total library duration, a short status
//! text, and (when enabled in Settings) the optional visualizer strip.

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
    egui::Panel::bottom("status_bar")
        .exact_size(28.0)
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(result_count_text(state, library));
                ui.separator();
                ui.label(format_duration(library.total_duration()));
                ui.separator();
                ui.label(status_text(player));
                shuffle_scope(ui, state, player);
                if let Some(message) = player.status_message() {
                    ui.separator();
                    ui.colored_label(ui.visuals().warn_fg_color, message);
                }
                scan_status(ui, state, library);
                auto_tag_status(ui, state, library);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if state.visualizer_enabled {
                        super::visualizer::show(ui, state, player);
                    }
                });
            });
        });
}

/// Shows the library's scan progress, with a cancel button while a scan is
/// running.
fn scan_status(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    let Some(text) = library.status_text() else {
        return;
    };
    ui.separator();
    ui.label(text);
    if library.is_scanning() && ui.small_button("Cancel").clicked() {
        state.push(Command::LibraryCancelScan);
    }
}

/// Shows an in-flight online auto-tag lookup, with a cancel button (#210).
fn auto_tag_status(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    let Some(status) = library.auto_tag_status() else {
        return;
    };
    ui.separator();
    ui.label(status.text);
    if ui.small_button("Cancel").clicked() {
        state.push(Command::CancelAutoTag);
    }
}

/// Track-count label: "N of M tracks" while the Music view's search box
/// has an active query, plain "M tracks" otherwise.
fn result_count_text(state: &AppState, library: &dyn LibraryDataSource) -> String {
    match state.search_result_count {
        Some(matched) => format!("{matched} of {} tracks", library.track_count()),
        None => format!("{} tracks", library.track_count()),
    }
}

fn status_text(player: &dyn PlayerApi) -> String {
    use crate::player_api::PlaybackStatus;
    match player.status() {
        PlaybackStatus::Playing => "Playing".to_string(),
        PlaybackStatus::Paused => "Paused".to_string(),
        PlaybackStatus::Stopped => "Ready".to_string(),
    }
}

/// Shows the active scoped shuffle (#57) and a way to stop it.
fn shuffle_scope(ui: &mut egui::Ui, state: &mut AppState, player: &dyn PlayerApi) {
    let Some(scope) = player.shuffle_scope() else {
        return;
    };
    ui.separator();
    ui.label(format!("Shuffling: {scope}"));
    if ui.small_button("Stop").clicked() {
        state.push(Command::PlayerToggleShuffle);
    }
}

fn format_duration(total: std::time::Duration) -> String {
    let secs = total.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    format!("{hours}h {minutes}m total")
}
