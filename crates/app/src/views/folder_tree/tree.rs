//! Recursive rendering of the Folders view's collapsible directory tree.
//!
//! Expansion state lives in egui's own memory (keyed by each node's path), so
//! the tree needs no extra bookkeeping in [`super::FolderTreeState`]. Each
//! node's context menu can start a scoped shuffle of that directory (#57).

use eframe::egui;

use crate::library_api::{DirNodeInfo, LibraryDataSource};
use crate::state::Command;

/// Horizontal indent added per tree level.
///
/// egui's default (`18.0`) is tuned for checkbox alignment, not for a narrow
/// tree panel; stacked across a few levels it leaves little room for folder
/// names and counts. A tighter indent keeps nesting legible while reclaiming
/// width in the 180-460px Folders panel (#162).
const LEVEL_INDENT: f32 = 12.0;

/// Renders `nodes` as a collapsible tree, highlighting `selected` and setting
/// it when a row is clicked. Returns a shuffle command if one was requested.
pub fn show(
    ui: &mut egui::Ui,
    nodes: &[DirNodeInfo],
    selected: &mut Option<String>,
    library: &dyn LibraryDataSource,
    recursive: bool,
) -> Option<Command> {
    // Applies to the whole tree: leaf rows and `CollapsingState` bodies both
    // read `indent` from the ui they render into.
    ui.spacing_mut().indent = LEVEL_INDENT;
    if nodes.is_empty() {
        ui.label(egui::RichText::new("No folders").weak());
        return None;
    }
    let mut command = None;
    for node in nodes {
        if let Some(cmd) = node_ui(ui, node, selected, library, recursive) {
            command = Some(cmd);
        }
    }
    command
}

fn node_ui(
    ui: &mut egui::Ui,
    node: &DirNodeInfo,
    selected: &mut Option<String>,
    library: &dyn LibraryDataSource,
    recursive: bool,
) -> Option<Command> {
    if node.children.is_empty() {
        return ui
            .horizontal(|ui| {
                ui.add_space(ui.spacing().indent);
                let response = row(ui, node, selected.as_deref());
                if response.clicked() {
                    *selected = Some(node.path.clone());
                }
                row_menu(&response, node, library, recursive)
            })
            .inner;
    }

    let id = ui.make_persistent_id(("folder_tree", &node.path));
    let mut body_command = None;
    let (_, header, _) =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| row(ui, node, selected.as_deref()))
            .body(|ui| {
                for child in &node.children {
                    if let Some(cmd) = node_ui(ui, child, selected, library, recursive) {
                        body_command = Some(cmd);
                    }
                }
            });
    if header.inner.clicked() {
        *selected = Some(node.path.clone());
    }
    row_menu(&header.inner, node, library, recursive).or(body_command)
}

fn row(ui: &mut egui::Ui, node: &DirNodeInfo, selected: Option<&str>) -> egui::Response {
    let text = format!("{} ({})", node.name, node.total_track_count);
    ui.selectable_label(selected == Some(node.path.as_str()), text)
        .on_hover_text(format!(
            "{}\n{} track(s) here, {} including subfolders",
            node.path, node.direct_track_count, node.total_track_count
        ))
}

fn row_menu(
    response: &egui::Response,
    node: &DirNodeInfo,
    library: &dyn LibraryDataSource,
    recursive: bool,
) -> Option<Command> {
    let mut command = None;
    response.context_menu(|ui| {
        if ui.button("Shuffle play").clicked() {
            command = Some(crate::shuffle::folder(library, &node.path, recursive));
            ui.close();
        }
    });
    command
}
