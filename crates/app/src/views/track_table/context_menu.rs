//! Right-click context menu for a track row: Play, Play next, Add to queue,
//! Star/Unstar, Open file location, Copy path, Properties.

use eframe::egui;

use crate::library_api::TrackInfo;

/// What the caller should do after a context menu item is chosen. Playback
/// actions are handed back as [`crate::state::Command`]s by the caller;
/// `Properties` is turned into the caller's dialog state; `CopyPath` and
/// `OpenFileLocation` are executed immediately here since they have no
/// effect on shared app state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    Play,
    PlayNext,
    AddToQueue,
    /// Flip the row's starred state (#131); the caller turns this into a
    /// [`crate::state::Command::ToggleStarred`].
    ToggleStar,
    /// The user asked to see the track's full metadata; the caller opens the
    /// Properties dialog for the row.
    Properties,
}

/// Shows the context menu for `response` (a track row's response), if the
/// user right-clicked it. Returns `Some` when a playback action was chosen;
/// `CopyPath`/`OpenFileLocation` are handled internally.
pub fn show(response: &egui::Response, track: &TrackInfo) -> Option<ContextAction> {
    let mut action = None;
    response.context_menu(|ui| {
        if ui.button("Play").clicked() {
            action = Some(ContextAction::Play);
            ui.close();
        }
        if ui.button("Play next").clicked() {
            action = Some(ContextAction::PlayNext);
            ui.close();
        }
        if ui.button("Add to queue").clicked() {
            action = Some(ContextAction::AddToQueue);
            ui.close();
        }
        ui.separator();
        let star_label = if track.starred { "Unstar" } else { "Star" };
        if ui.button(star_label).clicked() {
            action = Some(ContextAction::ToggleStar);
            ui.close();
        }
        ui.separator();
        if ui.button("Open file location").clicked() {
            open_file_location(&track.path);
            ui.close();
        }
        if ui.button("Copy path").clicked() {
            ui.ctx().copy_text(track.path.clone());
            ui.close();
        }
        ui.separator();
        if ui.button("Properties…").clicked() {
            action = Some(ContextAction::Properties);
            ui.close();
        }
    });
    action
}

/// Best-effort "reveal in Explorer". Mock data uses fake paths, so this is
/// expected to silently fail to find anything outside a real library; any
/// error is swallowed rather than surfaced, since there's no good UI to
/// report it through yet.
fn open_file_location(path: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.replace('/', "\\")))
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
}
