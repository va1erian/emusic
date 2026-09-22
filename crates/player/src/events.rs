//! Events emitted by [`crate::Player`] as playback progresses.
//!
//! Consumers (the app shell, later the stats module in #23) drain these
//! from [`crate::Player::events`] once per frame/tick.

use std::path::PathBuf;
use std::time::Duration;

use crate::error::PlayerError;
use crate::queue::RepeatMode;

/// Coarse transport state, mirroring what a UI needs to render play/pause
/// controls. Intentionally has the same shape as `emusic::PlayerApi`'s
/// `PlaybackStatus` so an adapter between the two is a 1:1 mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Something that happened in the player, for UI/stats consumers.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    /// A track started playing (either a fresh `replace_and_play`/`next`, or
    /// an automatic advance after the previous track ended).
    TrackStarted { path: PathBuf, queue_index: usize },
    /// The current track reached its natural end (before any auto-advance
    /// to the next one has happened).
    TrackEnded { path: PathBuf },
    /// Transport state changed (play/pause/stop).
    StateChanged(PlaybackState),
    /// The queue's contents, shuffle order or current position changed.
    QueueChanged,
    /// The repeat mode changed.
    RepeatModeChanged(RepeatMode),
    /// The shuffle setting changed.
    ShuffleChanged(bool),
    /// An operation failed; playback continues in whatever state it was in
    /// (or moves to `Stopped` if the failure was opening a new track).
    Error(PlayerError),
    /// Listening-accounting summary for a track that just stopped being
    /// current (ended naturally, was skipped, or playback was replaced).
    ///
    /// `completed` is `true` when `listened` reached at least half the
    /// track's duration, capped at 4 minutes — the same heuristic
    /// `emusic_core::PlayEvent::counts_as_play` exists to encode. This event
    /// intentionally stays path-based rather than reusing `PlayEvent`
    /// directly: the player has no `TrackId` (no library crate is wired in
    /// yet, see #12), so downstream code (the stats module, #23) is expected
    /// to resolve `path` to a `TrackId` and build a `PlayEvent` from it.
    PlayFinished {
        path: PathBuf,
        listened: Duration,
        completed: bool,
    },
}
