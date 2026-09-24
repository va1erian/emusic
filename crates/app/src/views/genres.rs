//! egui renderer for the "Genres" view (#104): a full-width table of genres
//! with per-genre track counts, whose row context menu can start a scoped
//! shuffle.
//!
//! All state and logic live in [`GenresView`] (`emusic-ui`); this module only
//! draws the table and maps its context menu to [`GenresMsg`]s.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use super::EguiView;
use crate::library_api::LibraryDataSource;
use crate::state::AppState;
use emusic_ui::views::genres::{GenresMsg, GenresView};
use emusic_ui::views::{Commands, Ctx};

const ROW_HEIGHT: f32 = 20.0;
const HEADER_HEIGHT: f32 = 22.0;
const NAME_MIN_WIDTH: f32 = 120.0;
const COUNT_COL_WIDTH: f32 = 72.0;

/// Draws the view, refreshing the model from `library`.
pub fn show(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    let cx = Ctx::with_library(&[], None, library);
    let mut out = Commands::new();
    state.genres.show(ui, "genres_table", &cx, &mut out);
    state.pending.extend(out.into_vec());
}

impl EguiView for GenresView {
    fn show(&mut self, ui: &mut egui::Ui, id_salt: &str, cx: &Ctx, out: &mut Commands) {
        self.refresh(cx);

        ui.label(egui::RichText::new(self.count_label()).weak());
        ui.separator();

        let mut messages: Vec<GenresMsg> = Vec::new();
        let available_height = ui.available_height();
        TableBuilder::new(ui)
            .id_salt(id_salt)
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
                body.rows(ROW_HEIGHT, self.len(), |mut row| {
                    let genre = &self.rows()[row.index()];
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
                            messages.push(GenresMsg::Shuffle(genre.name.clone()));
                            ui.close();
                        }
                    });
                });
            });

        for msg in messages {
            self.update(msg, cx, out);
        }
    }
}
