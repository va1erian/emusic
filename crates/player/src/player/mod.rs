//! [`Player`]: transport, queue and listen-accounting glue on top of an
//! [`AudioBackend`].
//!
//! Split by responsibility across this module's files:
//! - `mod.rs` (this file) — the [`Player`] type itself, construction,
//!   read-only state and queue management.
//! - [`transport`] — play/pause/stop/next/previous/seek/volume.
//! - [`loading`] — opening tracks off the UI thread and reacting to
//!   end-of-track/open-failure.
//! - [`accounting`] — listen-time bookkeeping.

mod accounting;
mod loading;
mod tracker;
mod transport;

use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::backend::{AudioBackend, BackendChannel, ChannelCapabilities, SeekSupport};
use crate::error::PlayerError;
use crate::events::{PlaybackState, PlayerEvent};
use crate::listen::ListenAccounting;
use crate::queue::{QueueSource, RepeatMode};

/// Maximum number of upcoming tracks materialised for the queue panel. Keeps
/// a 100k-track shuffle scope from ever building a full visible queue.
pub const UPCOMING_LIMIT: usize = 20;

/// The currently loaded (playing/paused/stopped-at-zero) track.
struct CurrentTrack {
    channel: Box<dyn BackendChannel>,
    /// Keeps the backend's end-of-track callback registered; never read.
    _end_guard: Box<dyn Any + Send>,
    path: PathBuf,
    /// The track's total length, or `None` when the backend can't report one
    /// (`ChannelCapabilities::duration_known` is `false`).
    duration: Option<Duration>,
    /// What the channel can do (duration/seek), for the transport bar (#192).
    capabilities: ChannelCapabilities,
}

/// A position (and play/pause intent) to apply to the next track that
/// finishes opening. Set by [`Player::replace_and_play_at`] so a restored
/// session resumes at the saved spot instead of the start.
#[derive(Debug, Clone, Copy)]
pub(super) struct PendingResume {
    pub position: Duration,
    pub play: bool,
}

/// Result of an in-flight, off-UI-thread `AudioBackend::open` call.
enum OpenMessage {
    Ready {
        path: PathBuf,
        queue_index: usize,
        channel: Box<dyn BackendChannel>,
        guard: Box<dyn Any + Send>,
    },
    Failed {
        path: PathBuf,
        error: PlayerError,
    },
}

/// Playback engine: owns the queue, the currently loaded backend channel,
/// and listen-time accounting; emits [`PlayerEvent`]s for a UI (or the
/// stats module, later) to consume.
///
/// # Off-thread file opening
/// A comment on the tracking issue notes that library files may live on
/// slow or offline network shares, and opening a stream must not block the
/// UI thread. Every [`AudioBackend::open`] call here therefore runs on a
/// short-lived worker thread (see [`loading`]); [`Player::tick`] picks up
/// the result (success or failure) once it arrives and never blocks itself.
pub struct Player {
    backend: Arc<dyn AudioBackend>,
    queue: QueueSource,
    /// Human-readable description of the active shuffle scope, if playback
    /// is a scoped shuffle (see [`Player::play_shuffled`]).
    shuffle_scope: Option<String>,
    state: PlaybackState,
    /// Linear UI volume, `0.0..=1.0` (see [`crate::volume`] for the curve
    /// applied before it reaches the backend).
    volume: f32,
    current: Option<CurrentTrack>,
    pending_open: Option<Receiver<OpenMessage>>,
    /// Set by [`Player::replace_and_play_at`] and consumed once that open
    /// completes, to seek to a saved position and decide whether to play.
    pending_resume: Option<PendingResume>,
    end_tx: Sender<()>,
    end_rx: Receiver<()>,
    events_tx: Sender<PlayerEvent>,
    events_rx: Receiver<PlayerEvent>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    listen: ListenAccounting,
    /// When listen accounting last accrued time; `None` while not playing.
    last_tick: Option<Instant>,
}

impl Player {
    /// Creates a player around `backend`. Nothing plays until
    /// [`Player::replace_and_play`] (or `enqueue` + `next`) is called.
    pub fn new(backend: Arc<dyn AudioBackend>) -> Self {
        let (end_tx, end_rx) = unbounded();
        let (events_tx, events_rx) = unbounded();
        Self {
            backend,
            queue: QueueSource::explicit(),
            shuffle_scope: None,
            state: PlaybackState::Stopped,
            volume: 1.0,
            current: None,
            pending_open: None,
            pending_resume: None,
            end_tx,
            end_rx,
            events_tx,
            events_rx,
            waker: None,
            listen: ListenAccounting::new(),
            last_tick: None,
        }
    }

    /// Registers a callback invoked whenever a new event is emitted, e.g.
    /// `egui::Context::request_repaint`. Deliberately takes a plain
    /// `Fn`, not an egui type, so this crate has no UI dependency.
    pub fn set_waker(&mut self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.waker = Some(waker);
    }

    /// The event stream; drain with `player.events().try_iter()` once per
    /// frame/tick, after calling [`Player::tick`].
    pub fn events(&self) -> &Receiver<PlayerEvent> {
        &self.events_rx
    }

    /// Advances internal bookkeeping: picks up a completed off-thread file
    /// open, drains end-of-track signals, and accrues listen time. Call
    /// this once per frame/tick before reading state.
    pub fn tick(&mut self) {
        self.process_pending_open();
        self.process_end_signals();
        self.accrue_listened_time();
    }

    // -- Read-only state -----------------------------------------------

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn current_path(&self) -> Option<&Path> {
        self.current.as_ref().map(|c| c.path.as_path())
    }

    pub fn position(&self) -> Option<Duration> {
        self.current
            .as_ref()
            .and_then(|c| c.channel.position().ok())
    }

    pub fn duration(&self) -> Option<Duration> {
        self.current.as_ref().and_then(|c| c.duration)
    }

    /// Whether the current track can be seeked. `false` for a track whose
    /// backend ignores seeks (e.g. SID), so the transport bar disables its
    /// slider instead of letting a drag be silently dropped (#192).
    pub fn seek_supported(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|c| c.capabilities.seek != SeekSupport::Unsupported)
    }

    /// Points the SID decoder at an HVSC Songlengths database (the file or an
    /// HVSC root to auto-detect it in), or clears it. The backend loads it
    /// lazily and off the UI thread; a missing or malformed file only costs
    /// the SID tune's real length (#192).
    pub fn set_songlengths_path(&self, path: Option<&Path>) {
        self.backend.set_songlengths_path(path);
    }

    /// Sets the fallback play length for SID tunes with no Songlengths entry
    /// (#192).
    pub fn set_sid_fallback_length(&self, length: Duration) {
        self.backend.set_sid_fallback_length(length);
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// FFT magnitude bins from the currently loaded channel, for the
    /// visualizer's spectrum mode (#25); `None` when nothing is loaded.
    pub fn fft(&self) -> Option<Vec<f32>> {
        self.current.as_ref().and_then(|c| c.channel.fft())
    }

    /// Decoded float samples from the currently loaded channel, for the
    /// visualizer's oscilloscope mode (#25); `None` when nothing is loaded.
    pub fn samples(&self) -> Option<Vec<f32>> {
        self.current.as_ref().and_then(|c| c.channel.samples())
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.queue.repeat_mode()
    }

    pub fn shuffle(&self) -> bool {
        self.queue.shuffle_enabled()
    }

    /// The label of the active scoped shuffle, if any (see
    /// [`Player::play_shuffled`]); `None` for ordinary playback.
    pub fn shuffle_scope(&self) -> Option<&str> {
        self.shuffle_scope.as_deref()
    }

    /// The whole queue in navigation order. For a shuffle scope this is the
    /// scope's original order, not the shuffled play order.
    pub fn queue_paths(&self) -> impl Iterator<Item = &Path> {
        self.queue.iter_paths()
    }

    /// The not-yet-played portion of the queue, each paired with its index
    /// into the original list — for a UI "up next" list where "remove"/
    /// "jump" (see [`Player::jump_to`]) need to address a specific track,
    /// not just its display position. Capped at [`UPCOMING_LIMIT`].
    pub fn upcoming(&self) -> Vec<(usize, PathBuf)> {
        self.queue.upcoming(UPCOMING_LIMIT)
    }

    // -- Queue management -------------------------------------------------

    /// Replaces the whole queue with an explicit list and starts playing
    /// `items[start_index]`, leaving any shuffle scope.
    pub fn replace_and_play(&mut self, items: Vec<PathBuf>, start_index: usize) {
        self.drop_current_and_account();
        self.shuffle_scope = None;
        let path = self.queue.replace(items, start_index);
        self.emit(PlayerEvent::QueueChanged);
        self.open_current_or_stop(path);
    }

    /// Like [`Player::replace_and_play`], but positions the opened track at
    /// `position` and, when `play` is `false`, leaves it loaded and paused
    /// rather than playing. Used to reopen the previous session on startup.
    ///
    /// The seek and play/pause intent are applied when the off-thread open
    /// completes (see [`Player::process_pending_open`]); a failed open drops
    /// them, so they never leak onto a later track.
    pub fn replace_and_play_at(
        &mut self,
        items: Vec<PathBuf>,
        start_index: usize,
        position: Duration,
        play: bool,
    ) {
        self.drop_current_and_account();
        self.shuffle_scope = None;
        let path = self.queue.replace(items, start_index);
        self.emit(PlayerEvent::QueueChanged);
        self.pending_resume = Some(PendingResume { position, play });
        self.open_current_or_stop(path);
    }

    /// Starts a lazy shuffled playback over `scope`, showing `label` as the
    /// active scope. Tracks are pulled from the scope on demand, so only a
    /// short preview ever reaches the queue panel; unreadable tracks are
    /// skipped rather than stopping playback.
    pub fn play_shuffled(&mut self, scope: Vec<PathBuf>, label: impl Into<String>) {
        let repeat = self.queue.repeat_mode();
        self.drop_current_and_account();
        let mut source = crate::queue::ShuffleSource::new(scope);
        source.set_repeat_mode(repeat);
        self.queue = QueueSource::Shuffle(source);
        self.shuffle_scope = Some(label.into());
        self.emit(PlayerEvent::QueueChanged);
        let first = self.queue.advance();
        self.open_current_or_stop(first);
    }

    /// Appends a track to the end of the queue without affecting playback.
    pub fn enqueue(&mut self, path: PathBuf) {
        self.queue.enqueue(path);
        self.emit(PlayerEvent::QueueChanged);
    }

    /// Inserts a track immediately after the currently playing item (or
    /// leaves it at the end if nothing is current), without affecting
    /// playback.
    pub fn play_next(&mut self, path: PathBuf) {
        self.queue.play_next(path);
        self.emit(PlayerEvent::QueueChanged);
    }

    /// Removes the track at `item_index`. If it was the playing track,
    /// playback moves on to whatever now occupies its slot (or stops).
    pub fn remove(&mut self, item_index: usize) {
        let was_current = self.queue.current().cloned();
        self.queue.remove(item_index);
        self.emit(PlayerEvent::QueueChanged);
        let now_current = self.queue.current().cloned();
        if was_current != now_current {
            self.drop_current_and_account();
            self.open_current_or_stop(now_current);
        }
    }

    /// Moves the track at `from` to `to` (indices into the original list).
    pub fn move_track(&mut self, from: usize, to: usize) {
        self.queue.move_track(from, to);
        self.emit(PlayerEvent::QueueChanged);
    }

    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.queue.set_repeat_mode(mode);
        self.emit(PlayerEvent::RepeatModeChanged(mode));
    }

    /// Enables/disables shuffle. Disabling it while a scoped shuffle is
    /// active ends the scope, leaving the current track as a one-item queue.
    pub fn set_shuffle(&mut self, enabled: bool) {
        if !enabled && self.queue.is_shuffle() {
            self.end_shuffle_scope();
            self.emit(PlayerEvent::ShuffleChanged(false));
            self.emit(PlayerEvent::QueueChanged);
            return;
        }
        self.queue.set_shuffle(enabled);
        self.emit(PlayerEvent::ShuffleChanged(enabled));
        self.emit(PlayerEvent::QueueChanged);
    }

    /// Jumps directly to `item_index` (an index into the original queue
    /// list, e.g. from [`Player::upcoming`]) and starts playing it,
    /// dropping whatever was currently loaded.
    pub fn jump_to(&mut self, item_index: usize) {
        self.drop_current_and_account();
        let path = self.queue.jump_to(item_index);
        self.emit(PlayerEvent::QueueChanged);
        self.open_current_or_stop(path);
    }

    // -- Internals shared across this module's files -----------------------

    /// Drops the shuffle scope, keeping the current track as a one-item
    /// explicit queue so playback isn't interrupted.
    fn end_shuffle_scope(&mut self) {
        let current = self.queue.current().cloned();
        let repeat = self.queue.repeat_mode();
        self.shuffle_scope = None;
        self.queue = QueueSource::explicit();
        self.queue.set_repeat_mode(repeat);
        if let Some(path) = current {
            self.queue.replace(vec![path], 0);
        }
    }

    pub(super) fn emit(&mut self, event: PlayerEvent) {
        let _ = self.events_tx.send(event);
        if let Some(waker) = &self.waker {
            waker();
        }
    }

    pub(super) fn set_state(&mut self, new_state: PlaybackState) {
        if self.state != new_state {
            self.state = new_state;
            self.emit(PlayerEvent::StateChanged(new_state));
        }
    }

    /// Starts opening `path`, or stops if `path` is `None` (queue ran out).
    pub(super) fn open_current_or_stop(&mut self, path: Option<PathBuf>) {
        match path {
            Some(path) => {
                let queue_index = self.queue.current_item_index().unwrap_or(0);
                self.start_open(path, queue_index);
            }
            None => self.set_state(PlaybackState::Stopped),
        }
    }
}
