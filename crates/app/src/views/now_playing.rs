//! "Now playing" view placeholder: bigger now-playing summary plus queue.
//! The full now-playing panel (artwork, module info) is issue #7.

use eframe::egui;

use crate::player_api::PlayerApi;

pub fn show(ui: &mut egui::Ui, player: &dyn PlayerApi) {
    match player.now_playing() {
        Some(np) => {
            ui.heading(&np.title);
            ui.label(egui::RichText::new(&np.artist).size(16.0));
            ui.label(egui::RichText::new(&np.album).weak());
            ui.add_space(8.0);
            ui.label(format!(
                "{} / {}",
                format_duration(player.position()),
                format_duration(np.duration)
            ));
        }
        None => {
            ui.label(egui::RichText::new("Nothing is playing.").weak());
        }
    }

    ui.add_space(12.0);
    ui.label(egui::RichText::new("QUEUE").small().weak());
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, entry) in player.queue().iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}.", i + 1));
                ui.label(&entry.title);
                ui.label(egui::RichText::new(format!("— {}", entry.artist)).weak());
            });
        }
    });
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
