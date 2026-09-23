//! "Genres" view: a full-width table of genres with per-genre track counts
//! (from the library snapshot, not recounted every frame). Each row's context
//! menu can start a scoped shuffle of that genre (#57).

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::library_api::LibraryDataSource;
use crate::state::AppState;

const ROW_HEIGHT: f32 = 20.0;
const HEADER_HEIGHT: f32 = 22.0;
const NAME_MIN_WIDTH: f32 = 120.0;
const COUNT_COL_WIDTH: f32 = 72.0;

pub fn show(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    let genres = library.genres();

    ui.label(egui::RichText::new(format!("{} genres", genres.len())).weak());
    ui.separator();

    let available_height = ui.available_height();
    TableBuilder::new(ui)
        .id_salt("genres_table")
        .striped(true)
        .sense(egui::Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .column(Column::remainder().at_least(NAME_MIN_WIDTH).clip(true))
        .column(Column::exact(COUNT_COL_WIDTH))
        .header(HEADER_HEIGHT, |mut header| {
            header.col(|ui| {
                ui.add(egui::Label::new(egui::RichText::new("Genre").strong()));
            });
            header.col(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(egui::RichText::new("Tracks").strong()));
                });
            });
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, genres.len(), |mut row| {
                let genre = &genres[row.index()];
                row.col(|ui| {
                    ui.add(egui::Label::new(&genre.name).truncate().selectable(false));
                });
                row.col(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(
                            egui::RichText::new(genre.track_count.to_string()).weak(),
                        ));
                    });
                });

                let response = row.response();
                response.context_menu(|ui| {
                    if ui.button("Shuffle play").clicked() {
                        state.push(crate::shuffle::genre(library, &genre.name));
                        ui.close();
                    }
                });
            });
        });
}
