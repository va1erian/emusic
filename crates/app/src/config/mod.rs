//! Persisted user settings (#8): a serde [`Config`] written as TOML to
//! `%APPDATA%\emusic\config.toml`, loaded at startup and saved on exit and
//! (debounced) while the app runs.

mod io;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::Duration;

use emusic_player::tracker::TrackerSettings;
use serde::{Deserialize, Serialize};

use crate::player_api::{PlaybackStatus, PlayerApi, RepeatMode};
use crate::state::{Accent, AppState, PanelVisibility, Theme, View, VisualizerMode};

pub use io::{ConfigError, config_path, load, save};

/// The playback session as it was when the app last closed (#190): the
/// track that was loaded, how far into it playback had got, and whether it
/// was playing. Written on exit only, so it never takes part in the
/// debounced settings save (see [`Config::capture`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastPlayed {
    /// Full path to the loaded track. Defaults to empty (and is then
    /// ignored) if a hand-edited config drops the field.
    #[serde(default)]
    pub path: PathBuf,
    /// Playback position, in seconds.
    #[serde(default)]
    pub position_secs: f64,
    /// Whether the track was playing (vs. paused/stopped) at exit.
    #[serde(default)]
    pub playing: bool,
}

impl LastPlayed {
    /// Snapshots the player's current session, or `None` when no track is
    /// loaded (nothing to resume).
    pub fn capture(player: &dyn PlayerApi) -> Option<Self> {
        let now = player.now_playing()?;
        if now.path.is_empty() {
            return None;
        }
        Some(Self {
            path: PathBuf::from(&now.path),
            position_secs: player.position().as_secs_f64(),
            playing: player.status() == PlaybackStatus::Playing,
        })
    }

    /// The saved position as a [`Duration`], clamped away from negative
    /// values (a hand-edited config could contain one).
    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(self.position_secs.max(0.0))
    }

    /// The track path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

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
    /// Reopen the last played track where it left off on startup (#190).
    /// On by default; turn it off to always start with an empty player.
    pub resume_playback: bool,
    /// The playback session captured when the app last closed (#190), if
    /// resuming is enabled. Restored on startup by [`Self::apply_to_player`].
    #[serde(default)]
    pub last_played: Option<LastPlayed>,
    /// Whether the status-bar visualizer strip (#25) is shown. Defaults to
    /// `false` so an idle/playing app never repaints continuously unless the
    /// user opts in.
    pub visualizer_enabled: bool,
    /// Visualizer strip mode (#25): spectrum, oscilloscope or off.
    pub visualizer: VisualizerMode,
    /// Library folders to scan at startup, edited in Settings → Library
    /// (#19); can also be set by hand in `config.toml`.
    #[serde(default)]
    pub library_folders: Vec<PathBuf>,
    /// Tracker module playback settings (Settings → Tracker playback).
    #[serde(default)]
    pub tracker_settings: TrackerSettings,
    /// Soundfont MIDI files are rendered with (Settings > Playback).
    #[serde(default)]
    pub midi_soundfont: Option<PathBuf>,
    /// Most recently used soundfonts, newest first, for quick switching
    /// (Settings > Playback).
    #[serde(default)]
    pub recent_soundfonts: Vec<PathBuf>,
    /// Path to the HVSC Songlengths database — the `Songlengths.md5` file
    /// itself or an HVSC root folder to auto-detect it in — used to give SID
    /// tunes their real length (#192). `None` means SID lengths are unknown.
    #[serde(default)]
    pub songlengths_path: Option<PathBuf>,
    /// Fallback play length, in seconds, for SID tunes with no Songlengths
    /// entry, so they still stop and the queue advances (#192).
    #[serde(default = "default_sid_fallback_secs")]
    pub sid_fallback_secs: u32,
}

/// Default SID fallback play length, matching the player's own default.
fn default_sid_fallback_secs() -> u32 {
    emusic_player::sid::DEFAULT_TUNE_LENGTH.as_secs() as u32
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
            resume_playback: true,
            last_played: None,
            visualizer_enabled: false,
            visualizer: VisualizerMode::default(),
            library_folders: Vec::new(),
            tracker_settings: TrackerSettings::default(),
            midi_soundfont: None,
            recent_soundfonts: Vec::new(),
            songlengths_path: None,
            sid_fallback_secs: default_sid_fallback_secs(),
        }
    }
}

impl Config {
    /// Snapshots the settings worth persisting from the live app state and
    /// player. Everything here round-trips through [`Self::apply_to_state`]
    /// / [`Self::apply_to_player`], so saving is lossless for these fields.
    ///
    /// The [`Self::last_played`] session is deliberately left `None`: its
    /// position changes every frame while playing, so including it would
    /// make the settings compare dirty continuously and rewrite the file
    /// every debounce interval. It is captured separately, on exit.
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
            resume_playback: state.resume_playback,
            last_played: None,
            visualizer_enabled: state.visualizer_enabled,
            visualizer: state.visualizer,
            library_folders: state.library_folders.clone(),
            tracker_settings: state.tracker_settings,
            midi_soundfont: state.midi_soundfont.clone(),
            recent_soundfonts: state.recent_soundfonts.clone(),
            songlengths_path: state.songlengths_path.clone(),
            sid_fallback_secs: state.sid_fallback_secs,
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
        state.resume_playback = self.resume_playback;
        state.visualizer_enabled = self.visualizer_enabled;
        state.visualizer = self.visualizer;
        state.library_folders = self.library_folders.clone();
        state.tracker_settings = self.tracker_settings;
        state.midi_soundfont = self.midi_soundfont.clone();
        state.recent_soundfonts = self.recent_soundfonts.clone();
        state.songlengths_path = self.songlengths_path.clone();
        state.sid_fallback_secs = self.sid_fallback_secs;
    }

    /// Restores the player fields (volume, repeat, shuffle, tracker
    /// settings) and, when resuming is enabled, the last session (#190).
    pub fn apply_to_player(&self, player: &mut dyn PlayerApi) {
        player.set_volume(self.volume);
        player.set_repeat_mode(self.repeat_mode);
        player.set_shuffle(self.shuffle);
        player.set_tracker_settings(&self.tracker_settings);
        player.set_midi_soundfont(self.midi_soundfont.as_deref());
        player.set_songlengths_path(self.songlengths_path.as_deref());
        player.set_sid_fallback_length(Duration::from_secs(u64::from(
            self.sid_fallback_secs.max(1),
        )));
        if self.resume_playback
            && let Some(session) = &self.last_played
            && !session.path().as_os_str().is_empty()
        {
            player.restore_track(session.path(), session.position(), session.playing);
        }
    }
}
