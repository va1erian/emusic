//! Central-area view router. Each variant of [`crate::state::View`] maps to
//! one module here; views are intentionally simple placeholders (a track
//! table, column browser, album grid etc. are later issues) that read
//! through [`LibraryDataSource`]/[`PlayerApi`] so they already exercise the
//! mock data end to end.

pub(crate) mod album_grid;
mod artists;
pub(crate) mod column_browser;
mod egui_view;
pub(crate) mod folder_tree;
pub(crate) mod folders;
mod genres;
pub(crate) mod history;
pub(crate) mod most_played;
mod music;
mod now_playing;
mod settings;
pub(crate) mod starred;
pub(crate) mod track_table;

pub(crate) use egui_view::EguiView;

use eframe::egui;

use crate::app::images::ImageCaches;
use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::search::SearchEngine;
use crate::state::{AppState, View};

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    images: &mut ImageCaches,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
    search: &SearchEngine,
) {
    let mut music = std::mem::take(&mut state.music);
    egui::CentralPanel::default().show(ui, |ui| {
        ui.heading(state.view.label());
        ui.add_space(4.0);
        // Views lay themselves out to the available width (tables shrink and
        // clip their columns), so a horizontal scrollbar only appears when a
        // view's content has an intrinsic minimum width larger than the
        // panel. Wrapping the view body keeps that content reachable when the
        // window (or the right panel) leaves little room.
        // The Albums view scrolls its grid and track table itself, so the outer
        // area only scrolls sideways; a vertical bar here would draw over theirs.
        let vertical = state.view != View::Albums;
        egui::ScrollArea::new([true, vertical])
            .id_salt("central_view_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| match state.view {
                View::Music => music::show(ui, state, &mut music, library, player, search),
                View::Albums => {
                    album_grid::show(ui, state, &mut images.album_thumbs, library, player)
                }
                View::Artists => artists::show(ui, state, library),
                View::Genres => genres::show(ui, state, library),
                View::Folders => folders::show(ui, state, library, player),

                View::Starred => starred::show(ui, state, library, player),
                View::MostPlayed => most_played::show(ui, state, library, player),
                View::History => history::show(ui, state, library, player),
                View::NowPlaying => now_playing::show(ui, state, library, player),
                View::Settings => settings::show(ui, state),
            });
    });
    state.music = music;
}
