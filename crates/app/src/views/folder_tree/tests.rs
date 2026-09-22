//! Unit tests for the Folders view's selection/filter model.

use crate::library_api::TrackInfo;

use super::FolderTreeState;

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
