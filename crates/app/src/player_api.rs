//! UI-facing player trait.
//!
//! Real audio backends (built on `crates/player` / `crates/bass`) and the
//! mock backend from #32 both implement [`PlayerApi`]. The shell only talks
//! to this trait, so it can be wired to a real player later without
//! touching panel/view code.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Repeat behaviour for the queue.
///
/// Serialized (lowercase, e.g. `off`) for the config file (#8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    /// Cycles to the next mode, for a single "repeat" toolbar button.
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

/// Coarse transport state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Minimal description of the track currently loaded in the player.
///
/// This intentionally does not depend on `emusic-core::Track` (not merged
/// yet); it is small and local, and can be replaced with a reference/handle
/// to a real track once the `core` crate lands.
#[derive(Debug, Clone, Default)]
pub struct NowPlayingInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Full path to the source file, used to look up richer metadata in the
    /// library and to load artwork.
    pub path: String,
    pub duration: Duration,
}

/// Live tracker-module metadata, returned by [`PlayerApi::module_info`].
///
/// Only tracker formats (MOD/XM/IT/S3M and friends) expose this; streamed
/// audio returns `None`.
#[derive(Debug, Clone, Default)]
pub struct ModuleInfo {
    pub name: String,
    pub format: String,
    pub channels: u32,
    pub orders: u32,
    pub current_order: u32,
    pub current_row: u32,
    pub message: String,
    pub instruments: Vec<String>,
    pub samples: Vec<String>,
}

/// One entry in the upcoming queue, shown in the right panel.
#[derive(Debug, Clone, Default)]
pub struct QueueEntry {
    pub title: String,
    pub artist: String,
}

/// Read+command trait the app shell uses to drive playback.
///
/// Implementations are expected to be cheap to poll every frame (`tick`,
/// the getters); no I/O should happen on the UI thread.
pub trait PlayerApi {
    /// Advance any time-based internal state (mock position, fake spectrum
    /// animation, ...). Called once per frame before the UI reads state.
    fn tick(&mut self, dt: Duration);

    fn status(&self) -> PlaybackStatus;
    fn now_playing(&self) -> Option<&NowPlayingInfo>;
    fn position(&self) -> Duration;
    fn duration(&self) -> Option<Duration>;
    fn volume(&self) -> f32;
    fn repeat_mode(&self) -> RepeatMode;
    fn shuffle(&self) -> bool;
    fn queue(&self) -> &[QueueEntry];

    /// Live tracker-module metadata, if the current track is a module.
    fn module_info(&self) -> Option<&ModuleInfo>;

    /// Normalized (0.0..=1.0) magnitude bins for the visualizer strip.
    fn spectrum(&self) -> &[f32];

    fn play_pause(&mut self);
    fn stop(&mut self);
    fn next(&mut self);
    fn previous(&mut self);
    fn seek(&mut self, position: Duration);
    fn set_volume(&mut self, volume: f32);
    fn set_repeat_mode(&mut self, mode: RepeatMode);
    fn set_shuffle(&mut self, enabled: bool);

    /// Jump to a queue entry by its current index and start playback.
    fn queue_jump(&mut self, index: usize);
    /// Remove a queue entry by its current index.
    fn queue_remove(&mut self, index: usize);
}
