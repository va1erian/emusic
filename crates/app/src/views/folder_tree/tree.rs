//! egui rendering of the Folders view's collapsible directory tree (#18,
//! #101).
//!
//! The selection and filter live in the [`FoldersView`] model; this module
//! only draws the tree and records intents as messages. Expansion state lives
//! in egui's own memory (keyed by each node's path), so the model needs no
//! extra bookkeeping for it. Each node's context menu can start a scoped
//! shuffle of that directory (#57).

use eframe::egui;

use emusic_ui::views::folders::{FoldersMsg, FoldersView};

/// Horizontal indent added per tree level.
///
/// egui's default (`18.0`) is tuned for checkbox alignment, not for a narrow
/// tree panel; stacked across a few levels it leaves little room for folder
/// names and counts. A tighter indent keeps nesting legible while reclaiming
/// width in the 180-460px Folders panel (#162).
const LEVEL_INDENT: f32 = 12.0;

/// Renders the library's directory tree, highlighting the model's selection
/// and recording clicks/menu choices as messages.
pub fn show(
    ui: &mut egui::Ui,
    nodes: &[crate::library_api::DirNodeInfo],
    view: &FoldersView,
    messages: &mut Vec<FoldersMsg>,
) {
    // Applies to the whole tree: leaf rows and `CollapsingState` bodies both
    // read `indent` from the ui they render into.
    ui.spacing_mut().indent = LEVEL_INDENT;
    if nodes.is_empty() {
        ui.label(egui::RichText::new("No folders").weak());
        return;
    }
    for node in nodes {
        node_ui(ui, node, 0, view, messages);
    }
}

fn node_ui(
    ui: &mut egui::Ui,
    node: &crate::library_api::DirNodeInfo,
    depth: usize,
    view: &FoldersView,
    messages: &mut Vec<FoldersMsg>,
) {
    if node.children.is_empty() {
        let response = ui
            .horizontal(|ui| {
                // Root-level leaves have no `CollapsingState` body indenting
                // them, so they need a manual nudge to align with root
                // folders' text past the arrow.
                if depth == 0 {
                    ui.add_space(ui.spacing().indent);
                }
                row(ui, node, view)
            })
            .inner;
        row_menu(&response, node, view, messages);
        return;
    }

    let id = ui.make_persistent_id(("folder_tree", &node.path));
    let (_, header, _) =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| row(ui, node, view))
            .body(|ui| {
                for child in &node.children {
                    node_ui(ui, child, depth + 1, view, messages);
                }
            });
    row_menu(&header.inner, node, view, messages);
}

fn row(
    ui: &mut egui::Ui,
    node: &crate::library_api::DirNodeInfo,
    view: &FoldersView,
) -> egui::Response {
    let text = format!("{} ({})", node.name, node.total_track_count);
    // `ui.selectable_label` doesn't truncate: an untruncated `Button`/`Label`
    // inside a plain (non-wrapping) horizontal layout requests its full
    // natural width regardless of the panel's bounds, and that overflow
    // then feeds back into `egui::Panel`'s own size measurement (its outer
    // rect is the *rendered* content rect, clamped only against the range's
    // *max*) — so a long real-library name silently overrode the width the
    // user had dragged the panel down to, and the panel could never shrink
    // narrower than its widest row.
    ui.add(
        egui::Button::selectable(view.selected.as_deref() == Some(node.path.as_str()), text)
            .truncate(),
    )
    .on_hover_text(format!(
        "{}\n{} track(s) here, {} including subfolders",
        node.path, node.direct_track_count, node.total_track_count
    ))
}

fn row_menu(
    response: &egui::Response,
    node: &crate::library_api::DirNodeInfo,
    view: &FoldersView,
    messages: &mut Vec<FoldersMsg>,
) {
    response.context_menu(|ui| {
        if ui.button("Shuffle play").clicked() {
            messages.push(FoldersMsg::Shuffle {
                path: node.path.clone(),
                recursive: view.include_subfolders,
            });
            ui.close();
        }
    });
}
