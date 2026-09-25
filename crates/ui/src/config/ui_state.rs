//! In-app UI state persisted across runs (#214), on top of the settings
//! [`Config`](super::Config) already stores.
//!
//! This is the "where was I looking" half of a session: the window geometry,
//! the top-bar search query and each view's selection/sort. It is captured
//! (and compared) on every tick, so it takes part in the debounced save.

use serde::{Deserialize, Serialize};

use crate::state::{AppState, WindowGeometry};
use crate::views::album_grid::models::{AlbumKey, AlbumSort};
use crate::views::column_browser::selection::PaneSelection;
use crate::views::track_table::sort::SortState;

/// Persisted UI state: window geometry plus the in-app selection/sort state
/// of the views that have it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiState {
    /// Window size/position/maximized, restored when the window is built.
    pub window: WindowGeometry,
    /// Independent visualization window (#303) size/position/maximized,
    /// restored when it is next opened.
    #[serde(default)]
    pub viz_window: WindowGeometry,
    /// Top-bar search query.
    pub search_query: String,
    /// Music view's track-table sort.
    pub music_sort: SortState,
    /// Music view's selected track ids, pruned to tracks that still exist.
    pub music_selection: Vec<u64>,
    /// Column-browser pane selections (#16).
    pub column_browser: ColumnBrowserSelection,
    /// Albums view's tile size, sort and selected album (#17).
    pub album_grid: AlbumGridUi,
    /// Folders view's selected directory + "include subfolders" (#18).
    pub folder_tree: FolderTreeUi,
}

/// The column-browser panes' selected facet values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnBrowserSelection {
    pub genres: PaneSelection,
    pub artists: PaneSelection,
    pub albums: PaneSelection,
}

/// Albums view state that isn't already a flat [`Config`](super::Config)
/// field: tile size, sort order and which album is expanded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlbumGridUi {
    pub tile_size: f32,
    pub sort: AlbumSort,
    pub selected: Option<AlbumKey>,
}

impl Default for AlbumGridUi {
    fn default() -> Self {
        Self {
            tile_size: crate::views::album_grid::DEFAULT_TILE_SIZE,
            sort: AlbumSort::default(),
            selected: None,
        }
    }
}

/// Folders view's selected directory and subfolder toggle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FolderTreeUi {
    pub selected: Option<String>,
    pub include_subfolders: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            window: WindowGeometry::default(),
            viz_window: WindowGeometry::default(),
            search_query: String::new(),
            music_sort: SortState::default(),
            music_selection: Vec::new(),
            column_browser: ColumnBrowserSelection::default(),
            album_grid: AlbumGridUi::default(),
            folder_tree: FolderTreeUi {
                selected: None,
                include_subfolders: true,
            },
        }
    }
}

impl UiState {
    /// Snapshots the UI state worth restoring from the live app state.
    pub fn capture(state: &AppState) -> Self {
        Self {
            window: state.window,
            viz_window: state.viz_window,
            search_query: state.search_query.clone(),
            music_sort: state.music.table.sort,
            music_selection: state.music.table.selection.selected_ids_sorted(),
            column_browser: ColumnBrowserSelection {
                genres: state.music.browser.genres.clone(),
                artists: state.music.browser.artists.clone(),
                albums: state.music.browser.albums.clone(),
            },
            album_grid: AlbumGridUi {
                tile_size: state.album_grid.tile_size,
                sort: state.album_grid.sort,
                selected: state.album_grid.selected.clone(),
            },
            folder_tree: FolderTreeUi {
                selected: state.folders.selected.clone(),
                include_subfolders: state.folders.include_subfolders,
            },
        }
    }

    /// Restores the persisted UI state onto `state`, before the library loads
    /// (stale selections are pruned by the table once tracks are known).
    pub fn apply_to_state(&self, state: &mut AppState) {
        state.window = self.window;
        state.viz_window = self.viz_window;
        state.search_query = self.search_query.clone();
        state.music.table.sort = self.music_sort;
        state
            .music
            .table
            .selection
            .restore_selected(self.music_selection.iter().copied());
        state.music.browser.genres = self.column_browser.genres.clone();
        state.music.browser.artists = self.column_browser.artists.clone();
        state.music.browser.albums = self.column_browser.albums.clone();
        state.album_grid.tile_size = self.album_grid.tile_size;
        state.album_grid.sort = self.album_grid.sort;
        state.album_grid.selected = self.album_grid.selected.clone();
        state.folders.selected = self.folder_tree.selected.clone();
        state.folders.include_subfolders = self.folder_tree.include_subfolders;
    }
}
