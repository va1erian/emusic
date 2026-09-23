//! "Folders" view (#18): a collapsible directory tree on the left, and the
//! track table for the selected folder on the right, with an "include
//! subfolders" toggle.

use eframe::egui;

use super::folder_tree;
use super::track_table::{self, TrackAction};
use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::PlayerApi;
use crate::state::{AppState, Command};

/// Renders the tree's own left panel. Shown as a real top-level panel
/// (sibling to the navigator/right panel) *before* the `CentralPanel` is
/// created, not nested inside the central view's `ScrollArea` — nesting a
/// resizable `Panel` inside a scroll area let it compute its docking rect
/// from the scroll content's (potentially offset) bounds instead of the
/// screen, which let it paint over the navigator column instead of stopping
/// at its edge.
pub fn tree_panel(ui: &mut egui::Ui, state: &mut AppState, library: &dyn LibraryDataSource) {
    let recursive = state.folder_tree.include_subfolders;
    let mut folder_command = None;
    egui::Panel::left("folder_tree")
        .resizable(true)
        .default_size(260.0)
        .size_range(180.0..=460.0)
        .show(ui, |ui| {
            // Vertical only (#163): directory rows must stay inside the
            // panel. Horizontal scrolling let a wide, deeply-indented row
            // paint past the panel's right edge and over the track table, and
            // is no longer needed now that the tree's indentation is small
            // (#162). Overlong names are clipped to the panel instead.
            egui::ScrollArea::vertical()
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
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
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
