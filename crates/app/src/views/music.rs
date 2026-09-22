//! "Music" view: the full library as a virtualized, sortable track table
//! (#15), filtered by the column browser (#16) and the top bar's search box.

use eframe::egui;

use super::column_browser;
use super::track_table::{self, TrackAction};
use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    if library.track_count() == 0 {
        empty_state(ui, state);
        return;
    }

    if state.column_browser.visible {
        column_browser::show(ui, &mut state.column_browser, library);
    }

    let query = state.search_query.to_lowercase();
    let tracks: Vec<&_> = library
        .tracks()
        .iter()
        .filter(|t| state.column_browser.matches(t))
        .filter(|t| {
            query.is_empty()
                || t.title.to_lowercase().contains(&query)
                || t.artist.to_lowercase().contains(&query)
                || t.album.to_lowercase().contains(&query)
        })
        .collect();

    ui.label(egui::RichText::new(format!("{} tracks", tracks.len())).weak());
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
            TrackAction::Play(id) => Command::PlayTrack(id),
            TrackAction::PlayNext(id) => Command::PlayTrackNext(id),
            TrackAction::AddToQueue(id) => Command::QueueTrack(id),
        });
    }
}

/// First-run (or emptied-library) state: nothing to list, so offer to add a
/// music folder right away. The text distinguishes "no folders configured"
/// from "folders configured but nothing scanned yet".
fn empty_state(ui: &mut egui::Ui, state: &mut AppState) {
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        ui.heading("Your library is empty");
        ui.add_space(8.0);
        let message = if state.library_folders.is_empty() {
            "Add a folder with your music to get started."
        } else {
            "No tracks found in your music folders yet."
        };
        ui.label(message);
        ui.add_space(12.0);
        if ui.button("Add music folder").clicked()
            && let Some(path) = rfd::FileDialog::new().pick_folder()
        {
            state.push(Command::LibraryAddFolder(path));
        }
    });
}

/// Matches the player's now-playing info back to a library track id, so the
/// table can highlight the right row. Path is the most reliable identifier
/// because the real player's [`NowPlayingInfo`](crate::player_api::NowPlayingInfo)
/// only carries file-stem metadata today.
fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library.track_by_path(&now_playing.path).map(|t| t.id)
}
