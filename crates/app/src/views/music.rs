//! "Music" view: the full library as a virtualized, sortable track table
//! (#15), filtered by the top bar's search box.

use eframe::egui;

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
    let query = state.search_query.to_lowercase();
    let tracks: Vec<&_> = library
        .tracks()
        .iter()
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

/// Matches the player's now-playing info back to a library track id, so the
/// table can highlight the right row. Path is the most reliable identifier
/// because the real player's [`NowPlayingInfo`](crate::player_api::NowPlayingInfo)
/// only carries file-stem metadata today.
fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library.track_by_path(&now_playing.path).map(|t| t.id)
}
