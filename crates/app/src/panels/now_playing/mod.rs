//! Real now-playing right panel: artwork, rich metadata, tracker module
//! info and the upcoming queue.
//!
//! Split into focused submodules to keep each file small; the panel state
//! (artwork cache, collapsible section flags) lives in
//! [`PanelState`] and is stored on [`crate::state::AppState`].

mod artwork;
mod metadata;
mod module_info;
mod queue;

use std::time::Duration;

use eframe::egui;

use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::state::AppState;

pub use artwork::ArtworkCache;

/// Persistent UI state for the now-playing panel.
#[derive(Default)]
pub struct PanelState {
    /// Cache for the current track's artwork texture.
    pub artwork: ArtworkCache,
}

/// Render the right-hand now-playing panel.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    egui::Panel::right("right_panel")
        .resizable(true)
        .default_size(260.0)
        .size_range(200.0..=440.0)
        .show(ui, |ui| {
            ui.add_space(4.0);

            let np = player.now_playing();
            let track = np.and_then(|info| library.track_by_path(&info.path));

            artwork::show(ui, &mut state.now_playing.artwork, np, track);

            match (np, track) {
                (Some(np), Some(track)) => {
                    ui.add_space(8.0);
                    metadata::show(ui, np, track, library);

                    if let Some(module) = player.module_info() {
                        ui.add_space(8.0);
                        ui.separator();
                        module_info::show(ui, module);
                    }
                }
                (Some(np), None) => {
                    // A track is loaded but not present in the library yet
                    // (e.g. a dragged-in file). Show the basic info we have.
                    ui.add_space(8.0);
                    metadata::show_basic(ui, np);
                }
                (None, _) => {
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Nothing playing").weak());
                }
            }

            ui.add_space(8.0);
            ui.separator();
            queue::show(ui, state, player);
        });
}

/// Format a duration as `m:ss`.
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
