//! Central-area view router. Each variant of [`crate::state::View`] maps to
//! one module here; views are intentionally simple placeholders (a track
//! table, column browser, album grid etc. are later issues) that read
//! through [`LibraryDataSource`]/[`PlayerApi`] so they already exercise the
//! mock data end to end.

pub(crate) mod album_grid;
mod artists;
pub(crate) mod column_browser;
pub(crate) mod folder_tree;
mod folders;
mod genres;
mod history;
mod most_played;
mod music;
mod now_playing;
mod settings;
pub(crate) mod track_table;

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::search::SearchEngine;
use crate::state::{AppState, View};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
    search: &SearchEngine,
) {
    egui::CentralPanel::default().show(ui, |ui| {
        ui.heading(state.view.label());
        ui.add_space(4.0);
        match state.view {
            View::Music => music::show(ui, state, library, player, search),
            View::Albums => album_grid::show(ui, state, library, player),
            View::Artists => artists::show(ui, state, library),
            View::Genres => genres::show(ui, state, library),
            View::Folders => folders::show(ui, state, library, player),
            View::MostPlayed => most_played::show(ui, library),
            View::History => history::show(ui, library),
            View::NowPlaying => now_playing::show(ui, player),
            View::Settings => settings::show(ui, state),
        }
    });
}
