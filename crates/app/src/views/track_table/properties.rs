//! Track "Properties" dialog (#136): the full metadata of one track, shown
//! as selectable text so any value can be copied. It replaces the accidental
//! per-cell text selection that used to be the only way to copy a tag - and
//! which stole clicks from the row underneath it.
//!
//! The sections and value formatting come from
//! [`emusic_ui::views::track_table::properties`] so the Win32 frontend's
//! native dialog shows exactly the same content (#280).

use eframe::egui;

use emusic_ui::views::track_table::properties::{self, PropertyField};

use crate::library_api::TrackInfo;

/// Shows the modal while `track` is `Some`, and clears it when the user
/// closes it (the Close button, Escape, or a click on the backdrop).
pub fn show(ctx: &egui::Context, track: &mut Option<TrackInfo>) {
    let Some(info) = track.clone() else {
        return;
    };

    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new("track_properties")).show(ctx, |ui| {
        ui.set_width(480.0);
        ui.heading("Properties");

        // Leave room for the heading, the Close button and the modal frame.
        let max_height = (ui.ctx().content_rect().height() - 140.0).max(200.0);
        egui::ScrollArea::vertical()
            .max_height(max_height)
            .show(ui, |ui| {
                for section in properties::sections(&info) {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(section.title.to_uppercase())
                            .small()
                            .strong()
                            .weak(),
                    );
                    egui::Grid::new(egui::Id::new(("track_properties", section.title)))
                        .num_columns(2)
                        .spacing([16.0, 5.0])
                        .show(ui, |ui| {
                            for PropertyField { label, value } in &section.fields {
                                field(ui, label, value);
                            }
                        });
                }
            });

        ui.add_space(12.0);
        if ui.button("Close").clicked() {
            close = true;
        }
    });

    if close || modal.should_close() {
        *track = None;
    }
}

/// One label/value pair in the grid. Values arrive display-ready from the
/// shared model (blank tags are already an em dash).
fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).strong());
    ui.add(egui::Label::new(value).selectable(true).wrap());
    ui.end_row();
}
