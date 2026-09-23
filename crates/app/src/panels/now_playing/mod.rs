//! Real now-playing right panel: artwork, rich metadata, tracker module
//! info and the upcoming queue.
//!
//! Split into focused submodules to keep each file small; the panel state
//! (artwork cache, collapsible section flags) lives in
//! [`PanelState`] and is stored on [`crate::state::AppState`].

mod artwork;
mod links;
pub(crate) mod metadata;
mod module_info;
pub(crate) mod queue;

use std::time::Duration;

use eframe::egui;

use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::PlayerApi;
use crate::state::AppState;

pub use artwork::ArtworkCache;
pub(crate) use artwork::load_artwork;

/// The panel's resizable width bounds.
const MIN_PANEL_WIDTH: f32 = 200.0;
const MAX_PANEL_WIDTH: f32 = 440.0;
const DEFAULT_PANEL_WIDTH: f32 = 260.0;
/// Width the central view keeps for itself: the right panel's maximum is
/// reduced by this much so narrowing the window can't squeeze the central
/// view (and the track table in it) down to nothing.
const MIN_CENTRAL_WIDTH: f32 = 320.0;

/// Persistent UI state for the now-playing panel.
#[derive(Default)]
pub struct PanelState {
    /// Cache for the current track's artwork texture.
    pub artwork: ArtworkCache,
    /// The track whose Properties dialog is open, if any. Rendered by the
    /// shell so it works whichever view is active.
    pub properties: Option<TrackInfo>,
}

/// Render the right-hand now-playing panel.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    let max_width =
        (ui.available_width() - MIN_CENTRAL_WIDTH).clamp(MIN_PANEL_WIDTH, MAX_PANEL_WIDTH);
    egui::Panel::right("right_panel")
        .resizable(true)
        .default_size(DEFAULT_PANEL_WIDTH)
        .size_range(MIN_PANEL_WIDTH..=max_width)
        .show(ui, |ui| {
            // The panel is a fixed, resizable width, but its content (long
            // queue entries especially) can be wider or taller than it. One
            // scroll area over the whole body keeps every part reachable
            // instead of clipping it.
            egui::ScrollArea::both()
                .id_salt("now_playing_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(4.0);

                    let np = player.now_playing();
                    let track = np.and_then(|info| library.track_by_path(&info.path));

                    artwork::show(ui, &mut state.now_playing.artwork, np, track);

                    match (np, track) {
                        (Some(np), Some(track)) => {
                            ui.add_space(8.0);
                            metadata::show(ui, np, track, library, state);

                            if let Some(module) = player.module_info() {
                                ui.add_space(8.0);
                                ui.separator();
                                module_info::show(ui, module);
                            }
                        }
                        (Some(np), None) => {
                            // A track is loaded but not present in the library
                            // yet (e.g. a dragged-in file). Show the basic info
                            // we have.
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
        });
}

/// Format a duration as `m:ss`.
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
