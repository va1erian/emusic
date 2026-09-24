//! Folder view selection/filter state (#18, #93).
//!
//! The collapsible directory tree itself is rendered by each frontend; this
//! module holds the persistent selection state the Folders view keeps across
//! frames.

use std::path::Path;

use crate::library_api::TrackInfo;

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

#[cfg(test)]
mod tests {
    //! Unit tests for the Folders view's selection/filter model, moved with
    //! the code (#93).

    use super::FolderTreeState;
    use crate::library_api::TrackInfo;

    fn track(path: &str) -> TrackInfo {
        TrackInfo {
            path: path.to_string(),
            ..TrackInfo::default()
        }
    }

    #[test]
    fn no_selection_matches_every_track() {
        let state = FolderTreeState::default();
        assert!(state.matches(&track(r"C:\music\a.flac")));
        assert!(state.matches(&track(r"D:\other\b.flac")));
    }

    #[test]
    fn including_subfolders_matches_descendants() {
        let state = FolderTreeState {
            selected: Some(r"C:\music".to_string()),
            include_subfolders: true,
        };
        assert!(state.matches(&track(r"C:\music\a.flac")));
        assert!(state.matches(&track(r"C:\music\Album\b.flac")));
        assert!(!state.matches(&track(r"C:\other\c.flac")));
    }

    #[test]
    fn excluding_subfolders_matches_only_direct_tracks() {
        let state = FolderTreeState {
            selected: Some(r"C:\music".to_string()),
            include_subfolders: false,
        };
        assert!(state.matches(&track(r"C:\music\a.flac")));
        assert!(!state.matches(&track(r"C:\music\Album\b.flac")));
    }

    #[test]
    fn prefix_is_matched_by_path_component_not_substring() {
        let state = FolderTreeState {
            selected: Some(r"C:\music\Lib".to_string()),
            include_subfolders: true,
        };
        assert!(!state.matches(&track(r"C:\music\Library\a.flac")));
        assert!(state.matches(&track(r"C:\music\Lib\a.flac")));
    }
}
