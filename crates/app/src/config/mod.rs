//! Persisted user settings (#8): a serde [`Config`] written as TOML to
//! `%APPDATA%\emusic\config.toml`, loaded at startup and saved on exit and
//! (debounced) while the app runs.

mod io;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::player_api::{PlayerApi, RepeatMode};
use crate::state::{Accent, AppState, PanelVisibility, Theme, View};

pub use io::{ConfigError, config_path, load, save};

/// Everything persisted across runs. Unknown fields in the file are
/// ignored and missing ones fall back to [`Config::default`], so configs
/// written by newer or older versions keep loading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Master volume, 0.0..=1.0.
    pub volume: f32,
    /// Queue repeat behaviour.
    pub repeat_mode: RepeatMode,
    /// Whether playback order is shuffled.
    pub shuffle: bool,
    /// Colour scheme.
    pub theme: Theme,
    /// UI accent colour (preset name or `#rrggbb`).
    pub accent: Accent,
    /// Which optional panels are visible.
    pub panels: PanelVisibility,
    /// Whether the Music view's column browser (#16) is shown.
    pub column_browser_visible: bool,
    /// Height of the column browser's splitter, in pixels.
    pub column_browser_height: f32,
    /// View shown on startup.
    pub last_view: View,
    /// Library folders to scan at startup. Editable UI is #19; until then
    /// users can set this in `config.toml`.
    #[serde(default)]
    pub library_folders: Vec<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            volume: 0.8,
            repeat_mode: RepeatMode::Off,
            shuffle: false,
            theme: Theme::default(),
            accent: Accent::default(),
            panels: PanelVisibility::default(),
            column_browser_visible: true,
            column_browser_height: crate::views::column_browser::DEFAULT_HEIGHT,
            last_view: View::default(),
            library_folders: Vec::new(),
        }
    }
}

impl Config {
    /// Snapshots the settings worth persisting from the live app state and
    /// player. Everything here round-trips through [`Self::apply_to_state`]
    /// / [`Self::apply_to_player`], so saving is lossless for these fields.
    pub fn capture(state: &AppState, player: &dyn PlayerApi) -> Self {
        Self {
            volume: player.volume(),
            repeat_mode: player.repeat_mode(),
            shuffle: player.shuffle(),
            theme: state.theme,
            accent: state.accent,
            panels: state.panels,
            column_browser_visible: state.column_browser.visible,
            column_browser_height: state.column_browser.height,
            last_view: state.view,
            library_folders: state.library_folders.clone(),
        }
    }

    /// Restores the UI-state fields (theme, accent, panels, column browser,
    /// last view).
    pub fn apply_to_state(&self, state: &mut AppState) {
        state.theme = self.theme;
        state.accent = self.accent;
        state.panels = self.panels;
        state.column_browser.visible = self.column_browser_visible;
        state.column_browser.height = self.column_browser_height;
        state.view = self.last_view;
        state.library_folders = self.library_folders.clone();
    }

    /// Restores the player fields (volume, repeat, shuffle).
    pub fn apply_to_player(&self, player: &mut dyn PlayerApi) {
        player.set_volume(self.volume);
        player.set_repeat_mode(self.repeat_mode);
        player.set_shuffle(self.shuffle);
    }
}
