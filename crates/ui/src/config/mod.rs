//! Persisted user settings (#8): a serde [`Config`] written as TOML to
//! `%APPDATA%\emusic\config.toml`, loaded at startup and saved on exit and
//! (debounced) while the app runs.

mod io;
#[cfg(test)]
mod tests;
mod ui_state;

use std::path::PathBuf;
use std::time::Duration;

use emusic_player::QueueSnapshot;
use emusic_player::tracker::TrackerSettings;
use serde::{Deserialize, Serialize};

use crate::player_api::{PlayerApi, RepeatMode};
use crate::state::projectm::{ProjectMSettings, VizLayout};
use crate::state::{Accent, AppState, PanelVisibility, Theme, View, VisualizerMode};

pub use io::{ConfigError, config_path, load, save};
pub use ui_state::UiState;

/// The playback session as it was when the app last closed (#190, #214): the
/// whole play queue (explicit or scoped shuffle) and how far into the current
/// track playback had got. Whether it starts playing on the next launch is
/// decided by [`Config::autoplay_on_restore`], not stored here.
///
/// Written on exit only, so it never takes part in the debounced settings
/// save (see [`Config::capture`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackSession {
    /// The whole queue (explicit list with its shuffle order, or scoped
    /// shuffle with its scope, history and remaining bag).
    pub queue: QueueSnapshot,
    /// Playback position, in seconds.
    pub position_secs: f64,
}

impl Default for PlaybackSession {
    fn default() -> Self {
        Self {
            queue: empty_queue(),
            position_secs: 0.0,
        }
    }
}

/// An empty explicit queue, for [`PlaybackSession`]'s serde default.
fn empty_queue() -> QueueSnapshot {
    QueueSnapshot::Explicit(emusic_player::ExplicitQueueSnapshot {
        items: Vec::new(),
        order: Vec::new(),
        pos: None,
        shuffle: false,
        repeat: emusic_player::RepeatMode::Off,
    })
}

impl PlaybackSession {
    /// Snapshots the player's current session, or `None` when nothing is
    /// loaded and the queue is empty (nothing to resume).
    pub fn capture(player: &dyn PlayerApi) -> Option<Self> {
        let queue = player.queue_snapshot();
        if player.now_playing().is_none() && queue.is_empty() {
            return None;
        }
        Some(Self {
            queue,
            position_secs: player.position().as_secs_f64(),
        })
    }

    /// The saved position as a [`Duration`], clamped away from negative
    /// values (a hand-edited config could contain one).
    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(self.position_secs.max(0.0))
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
    /// Reopen the last played track and queue where they left off on startup
    /// (#190, #214). On by default; turn it off to always start with an empty
    /// player.
    pub resume_playback: bool,
    /// Whether a restored session starts playing instead of coming back
    /// paused (#214). Off by default.
    pub autoplay_on_restore: bool,
    /// The playback session (whole queue + position) captured when the app
    /// last closed (#214), if resuming is enabled. Restored on startup by
    /// [`Self::apply_to_player`].
    #[serde(default)]
    pub last_session: Option<PlaybackSession>,
    /// Window geometry and the in-app UI state (search query, view
    /// selections/sorts) saved across runs (#214).
    #[serde(default)]
    pub ui: UiState,
    /// Whether the status-bar visualizer strip (#25) is shown. Defaults to
    /// `false` so an idle/playing app never repaints continuously unless the
    /// user opts in.
    pub visualizer_enabled: bool,
    /// Visualizer strip mode (#25): spectrum, oscilloscope or off.
    pub visualizer: VisualizerMode,
    /// Where the projectM visualization is shown (#300): visible, dock,
    /// fullscreen and its monitor.
    #[serde(default)]
    pub projectm_layout: VizLayout,
    /// projectM preset timing, sensitivity and preset selection (#300).
    #[serde(default)]
    pub projectm: ProjectMSettings,
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
            autoplay_on_restore: false,
            last_session: None,
            ui: UiState::default(),
            visualizer_enabled: false,
            visualizer: VisualizerMode::default(),
            projectm_layout: VizLayout::default(),
            projectm: ProjectMSettings::default(),
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
    /// The [`Self::last_session`] is deliberately left `None`: its position
    /// changes every frame while playing, so including it would make the
    /// settings compare dirty continuously and rewrite the file every
    /// debounce interval. It is captured separately, on exit.
    pub fn capture(state: &AppState, player: &dyn PlayerApi) -> Self {
        Self {
            volume: player.volume(),
            repeat_mode: player.repeat_mode(),
            shuffle: player.shuffle(),
            theme: state.theme,
            accent: state.accent,
            panels: state.panels,
            column_browser_visible: state.music.browser.visible,
            column_browser_height: state.music.browser.height,
            last_view: state.view,
            resume_playback: state.resume_playback,
            autoplay_on_restore: state.autoplay_on_restore,
            last_session: None,
            ui: UiState::capture(state),
            visualizer_enabled: state.visualizer_enabled,
            visualizer: state.visualizer,
            projectm_layout: state.projectm.layout.clone(),
            projectm: state.projectm.settings.clone(),
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
        state.music.browser.visible = self.column_browser_visible;
        state.music.browser.height = self.column_browser_height;
        state.view = self.last_view;
        state.resume_playback = self.resume_playback;
        state.autoplay_on_restore = self.autoplay_on_restore;
        self.ui.apply_to_state(state);
        state.visualizer_enabled = self.visualizer_enabled;
        state.visualizer = self.visualizer;
        state.projectm.layout = self.projectm_layout.clone();
        state.projectm.settings = self.projectm.sanitized();
        state.library_folders = self.library_folders.clone();
        state.tracker_settings = self.tracker_settings;
        state.midi_soundfont = self.midi_soundfont.clone();
        state.recent_soundfonts = self.recent_soundfonts.clone();
        state.songlengths_path = self.songlengths_path.clone();
        state.sid_fallback_secs = self.sid_fallback_secs;
    }

    /// Restores the player fields (volume, repeat, shuffle, tracker
    /// settings) and, when resuming is enabled, the last session's whole
    /// queue (#214). It starts playing only when
    /// [`Self::autoplay_on_restore`] is set; otherwise it comes back paused
    /// at the saved position.
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
            && let Some(session) = &self.last_session
            && !session.queue.is_empty()
        {
            player.restore_queue(&session.queue, session.position(), self.autoplay_on_restore);
        }
    }
}
