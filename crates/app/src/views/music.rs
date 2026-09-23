//! "Music" view: the full library as a virtualized, sortable track table
//! (#15), filtered by the column browser (#16) and the top bar's search box.

use eframe::egui;

use super::column_browser;
use super::track_table::{self, TrackAction};
use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::search::SearchEngine;
use crate::state::{AppState, Command};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
    search: &SearchEngine,
) {
    if library.track_count() == 0 {
        state.search_result_count = None;
        empty_state(ui, state, library);
        return;
    }

    if state.column_browser.visible {
        column_browser::show(ui, &mut state.column_browser, library);
    }

    // The query is parsed and matched off the UI thread (see
    // `crate::search`); here we only check each track's id against the
    // already-computed match set, which is O(1) per track.
    let tracks: Vec<&_> = library
        .tracks()
        .iter()
        .filter(|t| state.column_browser.matches(t))
        .filter(|t| search.is_match(t.id))
        .collect();

    state.search_result_count = if search.is_active() {
        Some(tracks.len())
    } else {
        None
    };

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{} tracks", tracks.len())).weak());
        if ui.button("Shuffle all").clicked() {
            state.push(crate::shuffle::all(library));
        }
    });
    ui.separator();

    let playing_id = currently_playing_id(library, player);
    let action = track_table::show(
        ui,
        "music_table",
        &mut state.music_table,
        &tracks,
        playing_id,
    );
    if let Some(action) = action {
        state.push(match action {
            TrackAction::Play { id, context } => Command::play_track(id, context),
            TrackAction::PlayNext(id) => Command::PlayTrackNext(id),
            TrackAction::AddToQueue(id) => Command::QueueTrack(id),
            TrackAction::ToggleStar(id) => Command::ToggleStarred(id),
            TrackAction::EditTags(id) => Command::OpenTagEditor(id),
        });
    }
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
