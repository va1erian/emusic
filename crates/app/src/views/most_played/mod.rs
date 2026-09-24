//! "Most Played" view (#24): tracks ranked by completed play count within a
//! selectable time window, reusing the shared virtualized track table (#15).
//!
//! The ranking itself is queried by the backend (per window); this view only
//! picks the window and renders the resulting list. The table's `#` column
//! doubles as the rank while the default (library) order is active.

use eframe::egui;

use super::track_table::{self, TrackAction};
use crate::library_api::{LibraryDataSource, StatsWindow, TrackInfo};
use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Window:").weak());
        for window in StatsWindow::ALL {
            let selected = state.most_played.window == window;
            if ui.selectable_label(selected, window.label()).clicked() {
                state.most_played.window = window;
            }
        }
    });
    ui.separator();

    let tracks = library.most_played(state.most_played.window);
    if tracks.is_empty() {
        ui.weak("No completed plays in this window yet.");
        return;
    }
    ui.label(egui::RichText::new(format!("Top {} tracks", tracks.len())).weak());
    ui.separator();

    let playing_id = currently_playing_id(library, player);
    let rows: Vec<&TrackInfo> = tracks.iter().collect();
    let action = track_table::show(
        ui,
        "most_played_table",
        &mut state.most_played.table,
        &rows,
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

/// Matches the player's now-playing info back to a library track id so the
/// table highlights the playing row; see the Music view's counterpart.
fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library.track_by_path(&now_playing.path).map(|t| t.id)
}
