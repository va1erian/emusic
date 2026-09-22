//! Right panel placeholder: now-playing artwork/metadata stub and the
//! upcoming queue. A full implementation lands in a later issue (now
//! playing panel); this establishes the layout slot.

use eframe::egui;

use crate::player_api::PlayerApi;

pub fn show(ui: &mut egui::Ui, player: &dyn PlayerApi) {
    egui::Panel::right("right_panel")
        .resizable(true)
        .default_size(230.0)
        .size_range(180.0..=400.0)
        .show(ui, |ui| {
            ui.add_space(4.0);
            artwork_placeholder(ui);

            match player.now_playing() {
                Some(np) => {
                    ui.add_space(6.0);
                    ui.heading(&np.title);
                    ui.label(&np.artist);
                    ui.label(egui::RichText::new(&np.album).weak());
                }
                None => {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Nothing playing").weak());
                }
            }

            ui.separator();
            ui.label(egui::RichText::new("UP NEXT").small().weak());
            egui::ScrollArea::vertical().show(ui, |ui| {
                for entry in player.queue() {
                    ui.horizontal(|ui| {
                        ui.label(&entry.title);
                        ui.label(egui::RichText::new(format!("— {}", entry.artist)).weak());
                    });
                }
            });
        });
}

fn artwork_placeholder(ui: &mut egui::Ui) {
    let size = egui::vec2(ui.available_width().min(200.0), 200.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "♪",
        egui::FontId::proportional(48.0),
        ui.visuals().weak_text_color(),
    );
}
