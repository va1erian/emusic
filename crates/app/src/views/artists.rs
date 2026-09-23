//! "Artists" view: a full-width table of artists with per-artist album and
//! track counts. Each row's context menu can start a scoped shuffle of that
//! artist (#57).

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::library_api::LibraryDataSource;
use crate::state::AppState;

const ROW_HEIGHT: f32 = 20.0;
const HEADER_HEIGHT: f32 = 22.0;
const NAME_MIN_WIDTH: f32 = 120.0;
const COUNT_COL_WIDTH: f32 = 72.0;

pub fn show(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    let mut artists: Vec<_> = library.artists().iter().collect();
    artists.sort_by(|a, b| a.name.cmp(&b.name));

    ui.label(egui::RichText::new(format!("{} artists", artists.len())).weak());
    ui.separator();

    let available_height = ui.available_height();
    TableBuilder::new(ui)
        .id_salt("artists_table")
        .striped(true)
        .sense(egui::Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .column(Column::remainder().at_least(NAME_MIN_WIDTH).clip(true))
        .column(Column::exact(COUNT_COL_WIDTH))
        .column(Column::exact(COUNT_COL_WIDTH))
        .header(HEADER_HEIGHT, |mut header| {
            header.col(|ui| {
                ui.add(egui::Label::new(egui::RichText::new("Artist").strong()));
            });
            header.col(|ui| count_header(ui, "Albums"));
            header.col(|ui| count_header(ui, "Tracks"));
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, artists.len(), |mut row| {
                let artist = artists[row.index()];
                row.col(|ui| {
                    ui.add(egui::Label::new(&artist.name).truncate().selectable(false));
                });
                row.col(|ui| count_cell(ui, artist.album_count));
                row.col(|ui| count_cell(ui, artist.track_count));

                let response = row.response();
                response.context_menu(|ui| {
                    if ui.button("Shuffle play").clicked() {
                        state.push(crate::shuffle::artist(library, &artist.name));
                        ui.close();
                    }
                });
            });
        });
}

/// A count column header, right-aligned to match its cells.
fn count_header(ui: &mut egui::Ui, label: &str) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add(egui::Label::new(egui::RichText::new(label).strong()));
    });
}

/// A right-aligned, weak numeric cell (the track table's convention for
/// secondary counts).
fn count_cell(ui: &mut egui::Ui, count: usize) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add(egui::Label::new(
            egui::RichText::new(count.to_string()).weak(),
        ));
    });
}
