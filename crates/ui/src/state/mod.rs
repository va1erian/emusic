//! Shared UI state (#94, #97): the toolkit-agnostic [`AppState`] shell plus
//! the smaller state types it is built from — appearance ([`Accent`],
//! [`Theme`], [`Palette`]), [`Command`] messages, [`View`] routing, panel
//! visibility, search popup, settings tabs and visualizer mode.

mod appearance;
mod command;
mod palette;
mod panels;
mod search;
mod settings;
mod view;
mod visualizer;
mod window;

pub use appearance::{Accent, DEFAULT_ACCENT, Rgb, Theme};
pub use command::Command;
pub use palette::{Palette, Rgba};
pub use panels::{PanelKind, PanelVisibility};
pub use search::{SearchPopupItem, SearchPopupState};
pub use settings::SettingsTab;
pub use view::View;
pub use visualizer::VisualizerMode;
pub use window::WindowGeometry;

use std::path::PathBuf;

use emusic_player::tracker::TrackerSettings;

use crate::views::album_grid::AlbumGrid;
use crate::views::folder_tree::FolderTreeState;
use crate::views::history::HistoryState;
use crate::views::most_played::MostPlayedState;
use crate::views::music::MusicView;
use crate::views::track_table::TrackTable;

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
    /// Whether closing and reopening the app should restore the last played
    /// track and position (#190). Mirrored from [`crate::config::Config`] so
    /// the setting round-trips through [`Config::capture`].
    ///
    /// [`Config::capture`]: crate::config::Config::capture
    pub resume_playback: bool,
    /// Whether a restored session (#214) starts playing instead of coming
    /// back paused. Off by default; mirrored from [`crate::config::Config`]
    /// so it round-trips.
    ///
    /// [`Config::capture`]: crate::config::Config::capture
    pub autoplay_on_restore: bool,
    /// Last known window geometry (#214), recorded by the frontend and
    /// applied when it builds the window on the next launch.
    pub window: WindowGeometry,
    /// Whether the status-bar visualizer strip (#25) is shown at all. Off by
    /// default: an animated strip needs a continuous repaint while playing,
    /// which costs CPU even when the user is not looking at it.
    pub visualizer_enabled: bool,
    /// Visualizer strip mode (#25), cycled by clicking the strip.
    pub visualizer: VisualizerMode,
    /// Transient visualizer rendering state (peak-hold caps), not persisted.
    pub visualizer_state: crate::panels::visualizer::VisualizerState,
    /// The Music view (#99): its column browser and track table, composed.
    pub music: MusicView,
    /// The Albums view's grid, sort and thumbnail cache (#17).
    pub album_grid: AlbumGrid,
    /// The Folders view's selected directory + "include subfolders" toggle
    /// (#18).
    pub folder_tree: FolderTreeState,
    /// The Folders view's track table (sort + selection).
    pub folders_table: TrackTable,
    /// The Starred view's track table (#131).
    pub starred_table: TrackTable,
    /// The Most Played view's window selector + track table (#24).
    pub most_played: MostPlayedState,
    /// The History view's confirmation flag (#24).
    pub history: HistoryState,
    /// Commands queued during this frame's `ui()`, drained at the end.
    pub pending: Vec<Command>,
    /// The open single-track tag editor dialog (#172), if any. Owned by the
    /// shell (not a track table) because the shell drains the backend's
    /// tag-edit results into it.
    pub tag_editor: Option<crate::tag_editor::TagEditorState>,
    /// Whether the File -> Database info dialog is open. Transient, not
    /// persisted.
    pub database_info_open: bool,
    /// Persistent state for the right-hand now-playing panel (artwork cache,
    /// collapsible section flags, ...).
    pub now_playing: crate::panels::now_playing::PanelState,
    /// Tracker module playback settings (Settings → Tracker playback),
    /// applied live to the player and persisted.
    pub tracker_settings: TrackerSettings,
    /// Soundfont (`.sf2`/`.sf3`/`.sfz`) MIDI files are rendered with
    /// (Settings > Playback); `None` falls back to one next to the BASS DLLs.
    pub midi_soundfont: Option<PathBuf>,
    /// Most recently used soundfonts, newest first, for quick switching
    /// (Settings > Playback). Capped at [`RECENT_SOUNDFONTS_LIMIT`].
    pub recent_soundfonts: Vec<PathBuf>,
    /// HVSC Songlengths database path (#192): the `Songlengths.md5` file or an
    /// HVSC root to auto-detect it in; `None` leaves SID lengths unknown.
    /// Mirrored from [`crate::config::Config`] so it round-trips.
    ///
    /// [`Config`]: crate::config::Config
    pub songlengths_path: Option<PathBuf>,
    /// Fallback play length, in seconds, for SID tunes with no Songlengths
    /// entry (#192).
    pub sid_fallback_secs: u32,
}

/// Maximum number of entries kept in [`AppState::recent_soundfonts`].
pub const RECENT_SOUNDFONTS_LIMIT: usize = 8;

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
            resume_playback: true,
            autoplay_on_restore: false,
            window: WindowGeometry::default(),
            visualizer_enabled: false,
            visualizer: VisualizerMode::default(),
            visualizer_state: crate::panels::visualizer::VisualizerState::default(),
            music: MusicView::default(),
            album_grid: AlbumGrid::default(),
            folder_tree: FolderTreeState::default(),
            folders_table: TrackTable::default(),
            starred_table: TrackTable::default(),
            most_played: MostPlayedState::default(),
            history: HistoryState::default(),
            pending: Vec::new(),
            tag_editor: None,
            database_info_open: false,
            now_playing: crate::panels::now_playing::PanelState::default(),
            tracker_settings: TrackerSettings::default(),
            midi_soundfont: None,
            recent_soundfonts: Vec::new(),
            songlengths_path: None,
            sid_fallback_secs: emusic_player::sid::DEFAULT_TUNE_LENGTH.as_secs() as u32,
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
            Command::GoToArtist(name) => {
                self.view = View::Artists;
                self.search_query = name.clone();
            }
            Command::GoToAlbum { name, artist } => {
                self.view = View::Albums;
                self.album_grid.select_album(name, artist);
            }
            Command::ToggleColumnBrowser => {
                self.music.browser.visible = !self.music.browser.visible;
            }
            Command::LibraryAddFolder(path) => {
                if !self.library_folders.contains(path) {
                    self.library_folders.push(path.clone());
                }
            }
            Command::LibraryRemoveFolder(path) => {
                self.library_folders.retain(|folder| folder != path);
            }
            Command::SetTrackerSettings(settings) => self.tracker_settings = *settings,
            Command::SetMidiSoundfont(path) => {
                if let Some(path) = path {
                    self.recent_soundfonts.retain(|recent| recent != path);
                    self.recent_soundfonts.insert(0, path.clone());
                    self.recent_soundfonts.truncate(RECENT_SOUNDFONTS_LIMIT);
                }
                self.midi_soundfont = path.clone();
            }
            Command::SetSonglengthsPath(path) => self.songlengths_path = path.clone(),
            Command::SetSidFallbackSecs(secs) => self.sid_fallback_secs = (*secs).max(1),
            _ => {}
        }
    }
}
