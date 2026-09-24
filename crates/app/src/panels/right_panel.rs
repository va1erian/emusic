//! Right panel entry point. The real implementation lives in the
//! `now_playing` module tree; this file is a thin wrapper that keeps the
//! existing call site in `app.rs` unchanged.

use eframe::egui;

use crate::app::images::ImageCaches;
use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::state::AppState;

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    images: &mut ImageCaches,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    crate::panels::now_playing::show(ui, state, &mut images.artwork, library, player);
}
