//! Upcoming queue list in the now-playing panel.
//!
//! The preview rows and the jump/remove intents live in the
//! [`NowPlayingView`] model (`emusic-ui`); this only draws them.

use eframe::egui;

use emusic_ui::views::now_playing::{NowPlayingMsg, NowPlayingView};

/// Show the upcoming queue with double-click jump and context-menu remove.
///
/// The caller provides the scroll context (the now-playing panel and the
/// Now Playing view each wrap their whole body in a scroll area), so this is
/// a plain list. Each row spans the full available width so the whole strip
/// is clickable.
pub fn show(ui: &mut egui::Ui, view: &NowPlayingView, messages: &mut Vec<NowPlayingMsg>) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("UP NEXT").small().weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(view.queue_count_text()).small().weak());
        });
    });

    for row in view.queue() {
        let response = ui
            .horizontal(|ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    egui::RichText::new(format!("{}.", row.number))
                        .monospace()
                        .weak(),
                );
                ui.label(&row.title);
                ui.label(egui::RichText::new(format!("— {}", row.artist)).weak());
            })
            .response;

        response.context_menu(|ui| {
            if ui.button("Remove").clicked() {
                messages.push(NowPlayingMsg::QueueRemove(row.index));
                ui.close();
            }
        });

        if response.double_clicked() {
            messages.push(NowPlayingMsg::QueueJump(row.index));
        }
    }
}
