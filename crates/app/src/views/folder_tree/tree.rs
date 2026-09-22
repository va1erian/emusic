//! Recursive rendering of the Folders view's collapsible directory tree.
//!
//! Expansion state lives in egui's own memory (keyed by each node's path), so
//! the tree needs no extra bookkeeping in [`super::FolderTreeState`].

use eframe::egui;

use crate::library_api::DirNodeInfo;

/// Renders `nodes` as a collapsible tree, highlighting `selected` and setting
/// it when a row is clicked.
pub fn show(ui: &mut egui::Ui, nodes: &[DirNodeInfo], selected: &mut Option<String>) {
    if nodes.is_empty() {
        ui.label(egui::RichText::new("No folders").weak());
        return;
    }
    for node in nodes {
        node_ui(ui, node, selected);
    }
}

fn node_ui(ui: &mut egui::Ui, node: &DirNodeInfo, selected: &mut Option<String>) {
    if node.children.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            if row(ui, node, selected.as_deref()).clicked() {
                *selected = Some(node.path.clone());
            }
        });
        return;
    }

    let id = ui.make_persistent_id(("folder_tree", &node.path));
    let (_, header, _) =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| row(ui, node, selected.as_deref()))
            .body(|ui| {
                for child in &node.children {
                    node_ui(ui, child, selected);
                }
            });
    if header.inner.clicked() {
        *selected = Some(node.path.clone());
    }
}

fn row(ui: &mut egui::Ui, node: &DirNodeInfo, selected: Option<&str>) -> egui::Response {
    let text = format!("{} ({})", node.name, node.total_track_count);
    ui.selectable_label(selected == Some(node.path.as_str()), text)
        .on_hover_text(format!(
            "{}\n{} track(s) here, {} including subfolders",
            node.path, node.direct_track_count, node.total_track_count
        ))
}
