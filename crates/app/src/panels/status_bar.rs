//! Bottom status bar: track count, total library duration, a short status
//! text, and a reserved strip for the visualizer (drawn from the player's
//! spectrum data; a real visualizer widget lands in a later issue).

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};
use crate::theme;

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
                scan_status(ui, state, library);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    visualizer_strip(ui, player);
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

fn visualizer_strip(ui: &mut egui::Ui, player: &dyn PlayerApi) {
    let size = egui::vec2(140.0, 18.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);

    let bins = player.spectrum();
    if bins.is_empty() {
        return;
    }
    let bar_width = rect.width() / bins.len() as f32;
    for (i, &v) in bins.iter().enumerate() {
        let x = rect.left() + i as f32 * bar_width;
        let height = rect.height() * v.clamp(0.0, 1.0);
        let bar = egui::Rect::from_min_max(
            egui::pos2(x, rect.bottom() - height),
            egui::pos2(x + bar_width * 0.8, rect.bottom()),
        );
        painter.rect_filled(bar, 0.0, theme::current_accent());
    }
}

fn format_duration(total: std::time::Duration) -> String {
    let secs = total.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    format!("{hours}h {minutes}m total")
}
