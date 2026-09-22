//! Bottom status bar: track count, total library duration, a short status
//! text, and a reserved strip for the visualizer (drawn from the player's
//! spectrum data; a real visualizer widget lands in a later issue).

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::theme::ACCENT;

pub fn show(ui: &mut egui::Ui, library: &dyn LibraryDataSource, player: &dyn PlayerApi) {
    egui::Panel::bottom("status_bar")
        .exact_size(28.0)
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(format!("{} tracks", library.track_count()));
                ui.separator();
                ui.label(format_duration(library.total_duration()));
                ui.separator();
                ui.label(status_text(player));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    visualizer_strip(ui, player);
                });
            });
        });
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
        painter.rect_filled(bar, 0.0, ACCENT);
    }
}

fn format_duration(total: std::time::Duration) -> String {
    let secs = total.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    format!("{hours}h {minutes}m total")
}
