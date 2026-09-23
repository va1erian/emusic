//! Upcoming queue list in the now-playing panel.

use eframe::egui;

use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};

/// Maximum number of upcoming tracks to show.
const PREVIEW_LIMIT: usize = 20;

/// Show the upcoming queue with double-click jump and context-menu remove.
///
/// The caller provides the scroll context (the now-playing panel and the
/// Now Playing view each wrap their whole body in a scroll area), so this is
/// a plain list. Each row spans the full available width so the whole strip
/// is clickable.
pub fn show(ui: &mut egui::Ui, state: &mut AppState, player: &dyn PlayerApi) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("UP NEXT").small().weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{} tracks", player.queue().len()))
                    .small()
                    .weak(),
            );
        });
    });

    for (i, entry) in player.queue().iter().enumerate().take(PREVIEW_LIMIT) {
        let response = ui
            .horizontal(|ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    egui::RichText::new(format!("{}.", i + 1))
                        .monospace()
                        .weak(),
                );
                ui.label(&entry.title);
                ui.label(egui::RichText::new(format!("— {}", entry.artist)).weak());
            })
            .response;

        response.context_menu(|ui| {
            if ui.button("Remove").clicked() {
                state.push(Command::PlayerQueueRemove(i));
                ui.close();
            }
        });

        if response.double_clicked() {
            state.push(Command::PlayerQueueJump(i));
        }
    }
}
