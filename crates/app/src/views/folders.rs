//! egui renderer for the "Folders" view (#18, #101): a collapsible directory
//! tree on the left, and the track table for the selected folder on the
//! right, with an "include subfolders" toggle.
//!
//! The selected directory, filter, tree rows and track table live in the
//! [`FoldersView`] model; this module only draws them.

use eframe::egui;

use super::EguiView;
use super::folder_tree;
use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::player_api::PlayerApi;
use crate::state::AppState;
use emusic_ui::views::folders::{FoldersMsg, FoldersView};
use emusic_ui::views::{Commands, Ctx};

/// The tree panel's resizable width bounds.
const MIN_TREE_WIDTH: f32 = 180.0;
/// Generous upper bound so a real library's deeper/longer paths (unlike the
/// short mock ones) have somewhere to grow; still bounded below by
/// [`MIN_CENTRAL_WIDTH`] so the track table can't be squeezed to nothing.
const MAX_TREE_WIDTH: f32 = 900.0;
const DEFAULT_TREE_WIDTH: f32 = 260.0;
/// Width the central view (track table) keeps for itself, same rationale as
/// the now-playing panel's own `MIN_CENTRAL_WIDTH`.
const MIN_CENTRAL_WIDTH: f32 = 320.0;

/// Renders the tree's own left panel. Shown as a real top-level panel
/// (sibling to the navigator/right panel) *before* the `CentralPanel` is
/// created, not nested inside the central view's `ScrollArea` — nesting a
/// resizable `Panel` inside a scroll area let it compute its docking rect
/// from the scroll content's (potentially offset) bounds instead of the
/// screen, which let it paint over the navigator column instead of stopping
/// at its edge.
pub fn tree_panel(
    ui: &mut egui::Ui,
    view: &mut FoldersView,
    library: &dyn LibraryDataSource,
) -> Commands {
    let mut messages = Vec::new();
    // `ui.available_width()` here already excludes the navigator and right
    // panel (shown earlier this frame), so this is genuinely the width left
    // to split between the tree and the track table.
    let max_width =
        (ui.available_width() - MIN_CENTRAL_WIDTH).clamp(MIN_TREE_WIDTH, MAX_TREE_WIDTH);
    egui::Panel::left("folder_tree")
        .resizable(true)
        .default_size(DEFAULT_TREE_WIDTH)
        .size_range(MIN_TREE_WIDTH..=max_width)
        .show(ui, |ui| {
            // Vertical only (#163): directory rows must stay inside the
            // panel. Horizontal scrolling let a wide, deeply-indented row
            // paint past the panel's right edge and over the track table, and
            // is no longer needed now that the tree's indentation is small
            // (#162). Overlong names are clipped to the panel instead.
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    folder_tree::show(ui, library.dir_tree(), view, &mut messages);
                });
        });
    let cx = Ctx::with_library(&[], None, library);
    let mut out = Commands::new();
    for msg in messages {
        view.update(msg, &cx, &mut out);
    }
    out
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    library: &dyn LibraryDataSource,
    player: &dyn PlayerApi,
) {
    let all: Vec<&TrackInfo> = library.tracks().iter().collect();
    let mut out = Commands::new();
    let cx_lib = Ctx::with_library(&all, None, library);

    let view = &mut state.folders;
    view.refresh(&cx_lib);
    let tracks: Vec<&TrackInfo> = view
        .visible_ids()
        .iter()
        .filter_map(|id| all.iter().copied().find(|track| track.id == *id))
        .collect();

    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(view.selected_label()).strong()).truncate());
        ui.separator();
        let mut include = view.include_subfolders;
        if ui.checkbox(&mut include, "Include subfolders").changed() {
            view.update(
                FoldersMsg::SetIncludeSubfolders(include),
                &Ctx::new(&tracks, None),
                &mut out,
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format!("{} tracks", tracks.len())).weak());
        });
    });
    ui.separator();

    let cx = Ctx::new(&tracks, currently_playing_id(library, player));
    view.table.show(ui, "folders_table", &cx, &mut out);
    state.pending.extend(out.into_vec());
}

/// Matches the player's now-playing info back to a library track id so the
/// table can highlight the playing row; see the Music view's counterpart.
fn currently_playing_id(library: &dyn LibraryDataSource, player: &dyn PlayerApi) -> Option<u64> {
    let now_playing = player.now_playing()?;
    library.track_by_path(&now_playing.path).map(|t| t.id)
}
