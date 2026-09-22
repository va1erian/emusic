//! Live tracker-module metadata display for the now-playing panel.

use eframe::egui;

use crate::player_api::ModuleInfo;

/// Show module name, format, channel/order count, current order/row, message
/// and an expandable instrument/sample list.
pub fn show(ui: &mut egui::Ui, module: &ModuleInfo) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("MODULE").small().weak());
        ui.label(egui::RichText::new(&module.format).small());
    });

    ui.label(format!(
        "{} · {} channels · {} orders",
        module.name, module.channels, module.orders
    ));
    ui.label(
        egui::RichText::new(format!(
            "Order {:02} / Row {:03}",
            module.current_order, module.current_row
        ))
        .monospace(),
    );

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
    ui.collapsing(
        format!(
            "Details ({} instruments, {} samples)",
            module.instruments.len(),
            module.samples.len()
        ),
        |ui| {
            details_list(ui, "Instruments", &module.instruments);
            details_list(ui, "Samples", &module.samples);
        },
    );
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
