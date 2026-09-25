//! Folders-view model (#18, #93, #101): the collapsible directory tree's
//! selection/filter state, the display rows built from the library's
//! directory grouping, and the selected folder's track table.
//!
//! The tree is rendered by the app (expansion state lives in its tree control);
//! this module holds the persistent selection, the flattened display rows, and
//! the "include subfolders" filter.

use std::path::Path;

use crate::library_api::{DirNodeInfo, TrackInfo};
use crate::state::Command;
use crate::views::track_table::{TrackTable, TrackTableMsg};
use crate::views::{Commands, Ctx};

/// A user intent on the Folders view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoldersMsg {
    /// A tree row was clicked; select that directory.
    SelectNode(String),
    /// Clear the selection, showing the whole library.
    SelectAll,
    /// Toggle whether the table includes the selected folder's subfolders.
    SetIncludeSubfolders(bool),
    /// The selected folder's tracks changed their sort/selection.
    Table(TrackTableMsg),
    /// Start a shuffled playback of `path`'s tracks.
    Shuffle { path: String, recursive: bool },
}

/// One row to draw in the directory tree, with its indentation depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub path: String,
    pub name: String,
    pub total_track_count: usize,
    pub direct_track_count: usize,
    /// Nesting depth, 0 for root folders, for the frontend to indent by.
    pub depth: usize,
    /// Whether this node has children (so the frontend draws a toggle).
    pub has_children: bool,
    /// Whether this row is the selected directory.
    pub selected: bool,
}

/// Persistent Folders-view state: the selected directory, the
/// "include subfolders" toggle, the selected folder's track table, and the
/// flattened tree rows rebuilt from the library each frame.
#[derive(Debug, Default)]
pub struct FoldersView {
    /// Selected directory path; `None` shows the whole library.
    pub selected: Option<String>,
    /// Whether the table includes tracks in the selected folder's
    /// subdirectories.
    pub include_subfolders: bool,
    /// The selected folder's track table (sort + selection).
    pub table: TrackTable,
    /// Flattened tree rows, rebuilt by [`FoldersView::refresh`].
    rows: Vec<TreeRow>,
    /// The tracks the table shows (the filter applied), as ids.
    visible_ids: Vec<u64>,
    /// Bumped whenever what the view displays changes.
    revision: u64,
}

impl FoldersView {
    /// Rebuilds the tree rows and the visible track ids from the library
    /// snapshot in `cx`.
    pub fn refresh(&mut self, cx: &Ctx) {
        let mut rows = Vec::new();
        if let Some(library) = cx.library {
            flatten(library.dir_tree(), 0, self.selected.as_deref(), &mut rows);
        }
        if rows != self.rows {
            self.rows = rows;
            self.revision += 1;
        }

        let visible_ids: Vec<u64> = cx
            .tracks
            .iter()
            .filter(|track| self.matches(track))
            .map(|track| track.id)
            .collect();
        if visible_ids != self.visible_ids {
            self.visible_ids = visible_ids;
            self.revision += 1;
        }
    }

    /// Whether `track` belongs in the table for the current selection.
    pub fn matches(&self, track: &TrackInfo) -> bool {
        let Some(selected) = &self.selected else {
            return true;
        };
        let path = Path::new(&track.path);
        if self.include_subfolders {
            path.starts_with(selected)
        } else {
            path.parent()
                .is_some_and(|parent| parent == Path::new(selected))
        }
    }

    /// The flattened tree rows to draw.
    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    /// The number of tracks shown by the table.
    pub fn visible_count(&self) -> usize {
        self.visible_ids.len()
    }

    /// The visible track ids, in library order.
    pub fn visible_ids(&self) -> &[u64] {
        &self.visible_ids
    }

    /// The label shown next to the toggle: the selected directory or "All
    /// folders".
    pub fn selected_label(&self) -> &str {
        self.selected.as_deref().unwrap_or("All folders")
    }

    /// The revision counter, bumped whenever the displayed state changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Applies one user intent, queueing any resulting commands. `cx` must
    /// carry the library snapshot ([`Ctx::with_library`]) and the visible
    /// tracks ([`FoldersView::visible_ids`]).
    pub fn update(&mut self, msg: FoldersMsg, cx: &Ctx, out: &mut Commands) {
        match msg {
            FoldersMsg::SelectNode(path) => {
                if self.selected.as_deref() != Some(path.as_str()) {
                    self.selected = Some(path);
                    self.revision += 1;
                }
            }
            FoldersMsg::SelectAll => {
                if self.selected.take().is_some() {
                    self.revision += 1;
                }
            }
            FoldersMsg::SetIncludeSubfolders(include) => {
                if self.include_subfolders != include {
                    self.include_subfolders = include;
                    self.revision += 1;
                }
            }
            FoldersMsg::Table(msg) => self.table.update(msg, cx, out),
            FoldersMsg::Shuffle { path, recursive } => {
                // Resolve the folder's track ids from the library snapshot
                // and hand the shell a scoped shuffle, matching the app's
                // `shuffle::folder` semantics (a path prefix when recursive,
                // an exact parent otherwise).
                if let Some(library) = cx.library {
                    let ids: Vec<u64> = library
                        .tracks()
                        .iter()
                        .filter(|track| folder_matches(&track.path, &path, recursive))
                        .map(|track| track.id)
                        .collect();
                    if !ids.is_empty() {
                        out.push(Command::ShuffleScope { ids, label: path });
                    }
                }
            }
        }
    }
}

/// Whether a track path is inside `folder`: a path-component prefix when
/// `recursive`, or an exact parent match otherwise. Mirrors the view's
/// `matches` on a bare path.
fn folder_matches(track_path: &str, folder: &str, recursive: bool) -> bool {
    let path = Path::new(track_path);
    if recursive {
        path.starts_with(folder)
    } else {
        path.parent()
            .is_some_and(|parent| parent == Path::new(folder))
    }
}

/// Flattens the directory tree into indented display rows.
fn flatten(nodes: &[DirNodeInfo], depth: usize, selected: Option<&str>, out: &mut Vec<TreeRow>) {
    for node in nodes {
        out.push(TreeRow {
            path: node.path.clone(),
            name: node.name.clone(),
            total_track_count: node.total_track_count,
            direct_track_count: node.direct_track_count,
            depth,
            has_children: !node.children.is_empty(),
            selected: selected == Some(node.path.as_str()),
        });
        flatten(&node.children, depth + 1, selected, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(path: &str) -> TrackInfo {
        TrackInfo {
            path: path.to_string(),
            ..TrackInfo::default()
        }
    }

    #[test]
    fn no_selection_matches_every_track() {
        let view = FoldersView::default();
        assert!(view.matches(&track(r"C:\music\a.flac")));
        assert!(view.matches(&track(r"D:\other\b.flac")));
    }

    #[test]
    fn including_subfolders_matches_descendants() {
        let view = FoldersView {
            selected: Some(r"C:\music".to_string()),
            include_subfolders: true,
            ..FoldersView::default()
        };
        assert!(view.matches(&track(r"C:\music\a.flac")));
        assert!(view.matches(&track(r"C:\music\Album\b.flac")));
        assert!(!view.matches(&track(r"C:\other\c.flac")));
    }

    #[test]
    fn excluding_subfolders_matches_only_direct_tracks() {
        let view = FoldersView {
            selected: Some(r"C:\music".to_string()),
            include_subfolders: false,
            ..FoldersView::default()
        };
        assert!(view.matches(&track(r"C:\music\a.flac")));
        assert!(!view.matches(&track(r"C:\music\Album\b.flac")));
    }

    #[test]
    fn prefix_is_matched_by_path_component_not_substring() {
        let view = FoldersView {
            selected: Some(r"C:\music\Lib".to_string()),
            include_subfolders: true,
            ..FoldersView::default()
        };
        assert!(!view.matches(&track(r"C:\music\Library\a.flac")));
        assert!(view.matches(&track(r"C:\music\Lib\a.flac")));
    }

    #[test]
    fn select_node_sets_and_reports_the_label() {
        let mut view = FoldersView::default();
        assert_eq!(view.selected_label(), "All folders");
        view.update(
            FoldersMsg::SelectNode(r"C:\music".to_string()),
            &Ctx::new(&[], None),
            &mut Commands::new(),
        );
        assert_eq!(view.selected_label(), r"C:\music");
        view.update(
            FoldersMsg::SelectAll,
            &Ctx::new(&[], None),
            &mut Commands::new(),
        );
        assert_eq!(view.selected_label(), "All folders");
    }
}
