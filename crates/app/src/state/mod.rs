//! Shared UI state: [`AppState`] plus the smaller state types the shell,
//! config and panels share. Each lives in its own module — the [`View`]
//! router, [`Theme`]/[`Accent`] appearance, panel [`PanelVisibility`], the
//! [`Command`] message type and the global [`SearchPopupState`] — and is
//! re-exported here so `crate::state::…` paths keep working.

mod appearance;
mod command;
mod panels;
mod search;
mod settings;
mod view;
mod visualizer;

pub use appearance::{Accent, Theme};
pub use command::Command;
pub use panels::{PanelKind, PanelVisibility};
pub use search::{SearchPopupItem, SearchPopupState};
pub use settings::SettingsTab;
pub use view::View;
pub use visualizer::VisualizerMode;

use std::path::PathBuf;

use crate::views::album_grid::AlbumGridState;
use crate::views::column_browser::ColumnBrowserState;
use crate::views::folder_tree::FolderTreeState;
use crate::views::history::HistoryState;
use crate::views::most_played::MostPlayedState;
use crate::views::track_table::TrackTableState;

#[cfg(test)]
mod tests;

/// Everything the shell needs beyond the player/library data itself.
pub struct AppState {
    pub view: View,
    pub theme: Theme,
    pub accent: Accent,
    pub panels: PanelVisibility,
    pub search_query: String,
    /// Number of tracks the Music view's search box currently matches;
    /// `None` when no query is active. Set by the Music view each frame,
    /// read by the status bar.
    pub search_result_count: Option<usize>,
    /// The global search popup (#22).
    pub search_popup: SearchPopupState,
    /// Library folders mirrored from [`crate::config::Config`] so the
    /// persisted list survives round-trips through [`Config::capture`].
    ///
    /// [`Config::capture`]: crate::config::Config::capture
    pub library_folders: Vec<PathBuf>,
    /// Settings sub-page shown while [`View::Settings`] is active (#137).
    /// Transient UI state, not persisted.
    ///
    /// [`View::Settings`]: crate::state::View
    pub settings_tab: SettingsTab,
    /// Visualizer strip mode (#25), cycled by clicking the strip.
    pub visualizer: VisualizerMode,
    /// Transient visualizer rendering state (peak-hold caps), not persisted.
    pub visualizer_state: crate::panels::visualizer::VisualizerState,
    /// The Music view's track table (sort + selection). Other views that
    /// embed a track table later (albums, artists, genres, folders,
    /// history) will each get their own field here.
    pub music_table: TrackTableState,
    /// The Music view's cascading filter panes (#16), above the track table.
    pub column_browser: ColumnBrowserState,
    /// The Albums view's grid, sort and thumbnail cache (#17).
    pub album_grid: AlbumGridState,
    /// The Folders view's selected directory + "include subfolders" toggle
    /// (#18).
    pub folder_tree: FolderTreeState,
    /// The Folders view's track table (sort + selection).
    pub folders_table: TrackTableState,
    /// The Starred view's track table (#131).
    pub starred_table: TrackTableState,
    /// The Most Played view's window selector + track table (#24).
    pub most_played: MostPlayedState,
    /// The History view's confirmation flag (#24).
    pub history: HistoryState,
    /// Commands queued during this frame's `ui()`, drained at the end.
    pub pending: Vec<Command>,
    /// Persistent state for the right-hand now-playing panel (artwork cache,
    /// collapsible section flags, ...).
    pub now_playing: crate::panels::now_playing::PanelState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            view: View::Music,
            theme: Theme::Dark,
            accent: Accent::default(),
            panels: PanelVisibility::default(),
            search_query: String::new(),
            search_result_count: None,
            search_popup: SearchPopupState::default(),
            library_folders: Vec::new(),
            settings_tab: SettingsTab::default(),
            visualizer: VisualizerMode::default(),
            visualizer_state: crate::panels::visualizer::VisualizerState::default(),
            music_table: TrackTableState::default(),
            column_browser: ColumnBrowserState::default(),
            album_grid: AlbumGridState::default(),
            folder_tree: FolderTreeState::default(),
            folders_table: TrackTableState::default(),
            starred_table: TrackTableState::default(),
            most_played: MostPlayedState::default(),
            history: HistoryState::default(),
            pending: Vec::new(),
            now_playing: crate::panels::now_playing::PanelState::default(),
        }
    }
}

impl AppState {
    pub fn push(&mut self, cmd: Command) {
        self.pending.push(cmd);
    }

    /// Applies every queued command to the local (non-player, non-library)
    /// bits of state. Player/library commands are applied by the caller,
    /// which owns those trait objects.
    pub fn apply_local(&mut self, cmd: &Command) {
        match cmd {
            Command::SetView(view) => self.view = *view,
            Command::ToggleTheme => self.theme = self.theme.toggled(),
            Command::SetAccent(accent) => self.accent = *accent,
            Command::CycleVisualizer => self.visualizer = self.visualizer.next(),
            Command::TogglePanel(kind) => match kind {
                PanelKind::Navigator => self.panels.navigator = !self.panels.navigator,
                PanelKind::RightPanel => self.panels.right_panel = !self.panels.right_panel,
                PanelKind::StatusBar => self.panels.status_bar = !self.panels.status_bar,
            },
            Command::SetSearchQuery(query) => self.search_query = query.clone(),
            Command::ToggleColumnBrowser => {
                self.column_browser.visible = !self.column_browser.visible;
            }
            Command::LibraryAddFolder(path) => {
                if !self.library_folders.contains(path) {
                    self.library_folders.push(path.clone());
                }
            }
            Command::LibraryRemoveFolder(path) => {
                self.library_folders.retain(|folder| folder != path);
            }
            _ => {}
        }
    }
}
