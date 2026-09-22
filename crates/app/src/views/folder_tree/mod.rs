//! Folder view state and the collapsible directory tree (#18).
//!
//! The tree itself is rendered by [`tree`]; this module holds the persistent
//! selection/filter state the Folders view keeps across frames.

#[cfg(test)]
mod tests;
mod tree;

use std::path::Path;

use crate::library_api::TrackInfo;

pub use tree::show;

/// Persistent Folders-view state: which directory is selected and whether the
/// track table also includes tracks from its subdirectories.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderTreeState {
    /// Selected directory path; `None` shows the whole library.
    pub selected: Option<String>,
    /// Whether the table includes tracks in the selected folder's
    /// subdirectories.
    pub include_subfolders: bool,
}

impl Default for FolderTreeState {
    fn default() -> Self {
        Self {
            selected: None,
            include_subfolders: true,
        }
    }
}

impl FolderTreeState {
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
}
