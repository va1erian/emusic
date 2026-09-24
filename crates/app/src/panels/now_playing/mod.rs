//! Real now-playing right panel: artwork, rich metadata, tracker module
//! info and the upcoming queue.
//!
//! The display data and intents live in the [`NowPlayingView`] model
//! (`emusic-ui`); this module only draws. The artwork texture cache is
//! egui-bound (it owns GPU handles via
//! [`EguiImageSink`](crate::image_sink::EguiImageSink), #96) and is passed in
//! by the frontend.

pub(crate) mod artwork;
mod links;
pub(crate) mod metadata;
mod module_info;
pub(crate) mod queue;

use eframe::egui;

use emusic_ui::views::Commands;
use emusic_ui::views::now_playing::{NowPlayingMsg, NowPlayingView};

use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::state::AppState;

use artwork::ArtworkCache;

/// The panel's resizable width bounds.
const MIN_PANEL_WIDTH: f32 = 200.0;
const MAX_PANEL_WIDTH: f32 = 440.0;
const DEFAULT_PANEL_WIDTH: f32 = 260.0;
/// Width the central view keeps for itself: the right panel's maximum is
/// reduced by this much so narrowing the window can't squeeze the central
/// view (and the track table in it) down to nothing.
const MIN_CENTRAL_WIDTH: f32 = 320.0;

/// Render the right-hand now-playing panel.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    artwork: &mut ArtworkCache,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    state.now_playing.refresh(player, library);
    let mut messages: Vec<NowPlayingMsg> = Vec::new();

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
                    body(ui, &state.now_playing, artwork, &mut messages);
                });
        });

    let mut out = Commands::new();
    for msg in messages {
        state.now_playing.update(msg, &mut out);
    }
    state.pending.extend(out.into_vec());
}

/// Draws the artwork, metadata/module info and queue from the model.
pub(crate) fn body(
    ui: &mut egui::Ui,
    view: &NowPlayingView,
    artwork: &mut ArtworkCache,
    messages: &mut Vec<NowPlayingMsg>,
) {
    artwork::show(ui, artwork, view);

    match (view.now_playing(), view.track()) {
        (Some(np), Some(track)) => {
            ui.add_space(8.0);
            metadata::show(ui, np, track, view, messages);

            if let Some(module) = view.module() {
                ui.add_space(8.0);
                ui.separator();
                module_info::show(ui, &module);
            }
        }
        (Some(np), None) => {
            // A track is loaded but not present in the library yet (e.g. a
            // dragged-in file). Show the basic info we have.
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
    queue::show(ui, view, messages);
}

/// Format a duration as `m:ss`.
pub fn format_duration(d: std::time::Duration) -> String {
    emusic_ui::views::now_playing::format_duration(d)
}
