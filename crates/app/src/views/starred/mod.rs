//! "Starred" view (#131): the tracks the user has favorited, reusing the
//! shared virtualized track table (#15) like Most Played / History do (#24).
//!
//! The star column is editable here and in every other track table, so
//! unstarring a row removes it from this list on the next frame.

use eframe::egui;

use super::EguiView;
use crate::library_api::LibraryDataSource;
use crate::player_api::PlayerApi;
use crate::state::AppState;
use emusic_ui::views::{Commands, Ctx};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    let tracks = library.starred_tracks();
    if tracks.is_empty() {
        ui.weak("No starred tracks yet. Click the star next to a track to add it here.");
        return;
    }

    ui.label(egui::RichText::new(format!("{} starred", tracks.len())).weak());
    ui.separator();

    let cx = Ctx::new(&tracks, currently_playing_id(library, player));
    let mut out = Commands::new();
    state.starred_table.show(ui, "starred_table", &cx, &mut out);
    state.pending.extend(out.into_vec());
}

/// Matches the player's now-playing info back to a library track id so the
/// table highlights the playing row; see the Music view's counterpart.
fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library.track_by_path(&now_playing.path).map(|t| t.id)
}
