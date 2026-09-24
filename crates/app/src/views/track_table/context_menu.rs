//! Right-click context menu for a track row: Play, Play next, Add to queue,
//! Star/Unstar, Open file location, Copy path, Edit tags, Properties.
//!
//! The state-affecting choices are the model's [`ContextAction`]s, handed
//! back to [`TrackTableMsg::Context`](emusic_ui::views::track_table::TrackTableMsg::Context);
//! Copy path / Open file location are executed immediately here since they
//! have no effect on shared app state.

use eframe::egui;

use emusic_ui::views::track_table::ContextAction;

use crate::library_api::TrackInfo;

/// Shows the context menu for `response` (a track row's response), if the
/// user right-clicked it. Returns `Some` when a state-affecting action was
/// chosen; Copy path / Open file location are handled internally.
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
        if ui.button("Edit tags…").clicked() {
            action = Some(ContextAction::EditTags);
            ui.close();
        }
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
