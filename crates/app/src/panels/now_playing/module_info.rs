//! Live tracker-module metadata display for the now-playing panel.

use eframe::egui;

use emusic_ui::views::now_playing::ModuleView;

/// Show module name, format, channel/order count, current order/row, message
/// and an expandable instrument/sample list.
pub fn show(ui: &mut egui::Ui, module: &ModuleView) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("MODULE").small().weak());
        ui.label(egui::RichText::new(&module.format).small());
    });

    ui.label(module.summary_text());
    ui.label(egui::RichText::new(module.order_row_text()).monospace());

    if !module.message.is_empty() {
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(&module.message)
                .small()
                .italics()
                .weak(),
        );
    }

    ui.add_space(2.0);
    ui.collapsing(module.details_label(), |ui| {
        details_list(ui, "Instruments", &module.instruments);
        details_list(ui, "Samples", &module.samples);
    });
}

fn details_list(ui: &mut egui::Ui, heading: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    ui.label(egui::RichText::new(heading).small().weak());
    egui::ScrollArea::vertical()
        .max_height(120.0)
        .show(ui, |ui| {
            for (i, item) in items.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{:02}", i + 1))
                            .monospace()
                            .weak(),
                    );
                    ui.label(item);
                });
            }
        });
}
