//! "Folders" view (#18): a collapsible directory tree on the left, and the
//! track table for the selected folder on the right, with an "include
//! subfolders" toggle.

use eframe::egui;

use super::folder_tree;
use super::track_table::{self, TrackAction};
use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    let recursive = state.folder_tree.include_subfolders;
    let mut folder_command = None;
    egui::Panel::left("folder_tree")
        .resizable(true)
        .default_size(260.0)
        .size_range(180.0..=460.0)
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    folder_command = folder_tree::show(
                        ui,
                        library.dir_tree(),
                        &mut state.folder_tree.selected,
                        library,
                        recursive,
                    );
                });
        });
    if let Some(command) = folder_command {
        state.push(command);
    }

    let tracks: Vec<&TrackInfo> = library
        .tracks()
        .iter()
        .filter(|track| state.folder_tree.matches(track))
        .collect();

    ui.horizontal(|ui| {
        let folder = state
            .folder_tree
            .selected
            .as_deref()
            .unwrap_or("All folders");
        ui.add(egui::Label::new(egui::RichText::new(folder).strong()).truncate());
        ui.separator();
        ui.checkbox(
            &mut state.folder_tree.include_subfolders,
            "Include subfolders",
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{} tracks", tracks.len())).weak());
        });
    });
    ui.separator();

    let playing_id = currently_playing_id(library, player);
    let action = track_table::show(
        ui,
        "folders_table",
        &mut state.folders_table,
        &tracks,
        playing_id,
    );
    if let Some(action) = action {
        state.push(match action {
            TrackAction::Play { id, context } => Command::play_track(id, context),
            TrackAction::PlayNext(id) => Command::PlayTrackNext(id),
            TrackAction::AddToQueue(id) => Command::QueueTrack(id),
            TrackAction::ToggleStar(id) => Command::ToggleStarred(id),
        });
    }
}

/// Matches the player's now-playing info back to a library track id so the
/// table can highlight the playing row; see the Music view's counterpart.
fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library.track_by_path(&now_playing.path).map(|t| t.id)
}
