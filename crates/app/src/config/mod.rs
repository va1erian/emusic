//! Persisted user settings (#8): a serde [`Config`] written as TOML to
//! `%APPDATA%\emusic\config.toml`, loaded at startup and saved on exit and
//! (debounced) while the app runs.

mod io;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::player_api::{PlayerApi, RepeatMode};
use crate::state::{AppState, PanelVisibility, Theme, View};

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
    /// Which optional panels are visible.
    pub panels: PanelVisibility,
    /// View shown on startup.
    pub last_view: View,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            volume: 0.8,
            repeat_mode: RepeatMode::Off,
            shuffle: false,
            theme: Theme::default(),
            panels: PanelVisibility::default(),
            last_view: View::default(),
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
            panels: state.panels,
            last_view: state.view,
        }
    }

    /// Restores the UI-state fields (theme, panels, last view).
    pub fn apply_to_state(&self, state: &mut AppState) {
        state.theme = self.theme;
        state.panels = self.panels;
        state.view = self.last_view;
    }

    /// Restores the player fields (volume, repeat, shuffle).
    pub fn apply_to_player(&self, player: &mut dyn PlayerApi) {
        player.set_volume(self.volume);
        player.set_repeat_mode(self.repeat_mode);
        player.set_shuffle(self.shuffle);
    }
}
