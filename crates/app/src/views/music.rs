//! egui renderer for the "Music" view (#15, #16, #99): the full library as a
//! virtualized, sortable track table, filtered by the column browser and the
//! top bar's search box.
//!
//! The column browser and table are both models in `emusic-ui`; this module
//! only draws them and routes messages.

use eframe::egui;

use super::EguiView;
use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::PlayerApi;
use crate::search::SearchEngine;
use crate::state::AppState;
use emusic_ui::views::music::{MusicMsg, MusicView};
use emusic_ui::views::{Commands, Ctx};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    music: &mut MusicView,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
    search: &SearchEngine,
) {
    if library.track_count() == 0 {
        state.search_result_count = None;
        empty_state(ui, state, library);
        return;
    }

    // The view model rebuilds the cascading facets and the filtered track
    // list from the library snapshot and the live search.
    let all: Vec<&TrackInfo> = library.tracks().iter().collect();
    music.refresh(&all, search);
    let tracks = music.visible_tracks(&all);
    state.search_result_count = music.search_result_count();
    // Keep the `Ctx`'s slice alive for the whole frame so the table can index
    // into exactly what `refresh` produced.
    let tracks: &[&TrackInfo] = &tracks;

    let mut out = Commands::new();
    let cx = Ctx::new(tracks, currently_playing_id(library, player));

    if music.browser.visible {
        music.browser.show(ui, "column_browser", &cx, &mut out);
    }

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{} tracks", tracks.len())).weak());
        if ui.button("Shuffle all").clicked() {
            music.update(MusicMsg::ShuffleAll, &cx, &mut out);
        }
    });
    ui.separator();

    music.table.show(ui, "music_table", &cx, &mut out);
    state.pending.extend(out.into_vec());
}

/// First-run (or emptied-library) state: nothing to list, so offer to add a
/// music folder right away. The text distinguishes "no folders configured"
/// from "folders configured but nothing scanned yet" — and, during a scan,
/// shows that the library is still being built plus the scan's progress
/// instead of a contradictory "no tracks found" message (#69, #80).
fn empty_state(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        let scanning = library.is_scanning() || library.status_text().is_some();
        ui.heading(empty_heading(scanning));
        ui.add_space(8.0);
        if scanning {
            ui.label(
                library
                    .status_text()
                    .unwrap_or_else(|| "Scanning your music folders...".to_string()),
            );
            ui.spinner();
            return;
        }
        let message = if state.library_folders.is_empty() {
            "Add a folder with your music to get started.".to_string()
        } else {
            "No tracks found in your music folders yet.".to_string()
        };
        ui.label(message);
        ui.add_space(12.0);
        if ui.button("Add music folder").clicked() {
            crate::settings::folder_picker::request();
        }
    });
}

/// Heading shown when the library has no tracks: while a scan is running the
/// library is being built, so calling it "empty" would be misleading (#80).
fn empty_heading(scanning: bool) -> &'static str {
    if scanning {
        "Building your music library..."
    } else {
        "Your library is empty"
    }
}

/// Matches the player's now-playing info back to a library track id, so the
/// table can highlight the right row. Path is the most reliable identifier
/// because the real player's [`NowPlayingInfo`](crate::player_api::NowPlayingInfo)
/// only carries file-stem metadata today.
fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library.track_by_path(&now_playing.path).map(|t| t.id)
}
