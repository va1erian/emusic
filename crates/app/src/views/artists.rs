//! egui renderer for the "Artists" view (#104): a full-width table of artists
//! with per-artist album and track counts, whose row context menu can start a
//! scoped shuffle.
//!
//! All state and logic live in [`ArtistsView`] (`emusic-ui`); this module only
//! draws the table and maps its context menu to [`ArtistsMsg`]s.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use super::EguiView;
use crate::library_api::LibraryDataSource;
use crate::state::AppState;
use emusic_ui::views::artists::{ArtistsMsg, ArtistsView};
use emusic_ui::views::{Commands, Ctx};

const ROW_HEIGHT: f32 = 20.0;
const HEADER_HEIGHT: f32 = 22.0;
const NAME_MIN_WIDTH: f32 = 120.0;
const COUNT_COL_WIDTH: f32 = 72.0;

/// Draws the view, refreshing the model from `library`.
pub fn show(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    let cx = Ctx::with_library(&[], None, library);
    let mut out = Commands::new();
    state.artists.show(ui, "artists_table", &cx, &mut out);
    state.pending.extend(out.into_vec());
}

impl EguiView for ArtistsView {
    fn show(&mut self, ui: &mut egui::Ui, id_salt: &str, cx: &Ctx, out: &mut Commands) {
        self.refresh(cx);

        ui.label(egui::RichText::new(self.count_label()).weak());
        ui.separator();

        let mut messages: Vec<ArtistsMsg> = Vec::new();
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
            .column(Column::exact(COUNT_COL_WIDTH))
            .header(HEADER_HEIGHT, |mut header| {
                header.col(|ui| {
                    header_cell(ui, "Artist");
                });
                header.col(|ui| count_header(ui, "Albums"));
                header.col(|ui| count_header(ui, "Tracks"));
            })
            .body(|body| {
                body.rows(ROW_HEIGHT, self.len(), |mut row| {
                    let artist = &self.rows()[row.index()];
                    row.col(|ui| {
                        ui.add(egui::Label::new(&artist.name).truncate().selectable(false));
                    });
                    row.col(|ui| count_cell(ui, artist.album_count));
                    row.col(|ui| count_cell(ui, artist.track_count));

                    let response = row.response();
                    response.context_menu(|ui| {
                        if ui.button("Shuffle play").clicked() {
                            messages.push(ArtistsMsg::Shuffle(artist.name.clone()));
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

/// A left-aligned, strong header cell.
fn header_cell(ui: &mut egui::Ui, label: &str) {
    ui.add(egui::Label::new(egui::RichText::new(label).strong()));
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
