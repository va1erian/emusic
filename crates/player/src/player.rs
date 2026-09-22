//! [`Player`]: transport, queue and listen-accounting glue on top of an
//! [`AudioBackend`].

use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};

use crate::backend::{AudioBackend, BackendChannel};
use crate::error::PlayerError;
use crate::events::{PlaybackState, PlayerEvent};
use crate::listen::ListenAccounting;
use crate::queue::{Queue, RepeatMode};
use crate::volume::perceptual_to_gain;

/// A manual "previous" skips to the actual previous track only if playback
/// is within this long of the start; otherwise it restarts the current one.
const PREVIOUS_RESTART_THRESHOLD: Duration = Duration::from_secs(3);

/// The currently loaded (playing/paused/stopped-at-zero) track.
struct CurrentTrack {
    channel: Box<dyn BackendChannel>,
    /// Keeps the backend's end-of-track callback registered; never read.
    _end_guard: Box<dyn Any + Send>,
    path: PathBuf,
    duration: Option<Duration>,
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
/// short-lived worker thread; [`Player::tick`] picks up the result
/// (success or failure) once it arrives and never blocks itself.
pub struct Player {
    backend: Arc<dyn AudioBackend>,
    queue: Queue,
    state: PlaybackState,
    /// Linear UI volume, `0.0..=1.0` (see [`crate::volume`] for the curve
    /// applied before it reaches the backend).
    volume: f32,
    current: Option<CurrentTrack>,
    pending_open: Option<Receiver<OpenMessage>>,
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
            queue: Queue::new(),
            state: PlaybackState::Stopped,
            volume: 1.0,
            current: None,
            pending_open: None,
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

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.queue.repeat_mode()
    }

    pub fn shuffle(&self) -> bool {
        self.queue.shuffle_enabled()
    }

    /// The upcoming queue in navigation (shuffled, if enabled) order.
    pub fn queue_paths(&self) -> impl Iterator<Item = &Path> {
        self.queue.iter_order()
    }

    // -- Queue management -------------------------------------------------

    /// Replaces the whole queue and starts playing `items[start_index]`.
    pub fn replace_and_play(&mut self, items: Vec<PathBuf>, start_index: usize) {
        self.drop_current_and_account();
        let path = self.queue.replace(items, start_index);
        self.emit(PlayerEvent::QueueChanged);
        self.open_current_or_stop(path);
    }

    /// Appends a track to the end of the queue without affecting playback.
    pub fn enqueue(&mut self, path: PathBuf) {
        self.queue.enqueue(path);
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

    pub fn set_shuffle(&mut self, enabled: bool) {
        self.queue.set_shuffle(enabled);
        self.emit(PlayerEvent::ShuffleChanged(enabled));
        self.emit(PlayerEvent::QueueChanged);
    }

    // -- Transport ---------------------------------------------------------

    /// Toggles play/pause. No-op if nothing is loaded.
    pub fn play_pause(&mut self) {
        let Some(current) = &self.current else {
            return;
        };
        let result = match self.state {
            PlaybackState::Playing => current.channel.pause(),
            PlaybackState::Paused | PlaybackState::Stopped => current.channel.play(false),
        };
        if let Err(error) = result {
            self.emit(PlayerEvent::Error(error));
            return;
        }
        match self.state {
            PlaybackState::Playing => {
                self.last_tick = None;
                self.set_state(PlaybackState::Paused);
            }
            PlaybackState::Paused | PlaybackState::Stopped => {
                self.last_tick = Some(Instant::now());
                self.set_state(PlaybackState::Playing);
            }
        }
    }

    /// Stops playback and resets position to the start, keeping the track
    /// loaded (a subsequent `play_pause` resumes it from zero).
    pub fn stop(&mut self) {
        if let Some(current) = &self.current
            && let Err(error) = current.channel.stop()
        {
            self.emit(PlayerEvent::Error(error));
        }
        self.last_tick = None;
        self.set_state(PlaybackState::Stopped);
    }

    /// Skips to the next track per the queue's repeat mode.
    pub fn next(&mut self) {
        self.drop_current_and_account();
        let next = self.queue.advance();
        self.open_current_or_stop(next);
    }

    /// Skips to the previous track, or restarts the current one if more
    /// than [`PREVIOUS_RESTART_THRESHOLD`] has played.
    pub fn previous(&mut self) {
        if let Some(current) = &self.current
            && current.channel.position().unwrap_or_default() > PREVIOUS_RESTART_THRESHOLD
        {
            if let Err(error) = current.channel.seek(Duration::ZERO) {
                self.emit(PlayerEvent::Error(error));
            }
            return;
        }
        self.drop_current_and_account();
        let previous = self.queue.retreat();
        self.open_current_or_stop(previous);
    }

    /// Seeks the current track to `position`.
    pub fn seek(&mut self, position: Duration) {
        if let Some(current) = &self.current
            && let Err(error) = current.channel.seek(position)
        {
            self.emit(PlayerEvent::Error(error));
        }
    }

    /// Sets linear volume (`0.0..=1.0`); the perceptual curve is applied
    /// internally before reaching the backend.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(current) = &self.current
            && let Err(error) = current.channel.set_volume(perceptual_to_gain(self.volume))
        {
            self.emit(PlayerEvent::Error(error));
        }
    }

    // -- Internals -----------------------------------------------------

    fn emit(&mut self, event: PlayerEvent) {
        let _ = self.events_tx.send(event);
        if let Some(waker) = &self.waker {
            waker();
        }
    }

    fn set_state(&mut self, new_state: PlaybackState) {
        if self.state != new_state {
            self.state = new_state;
            self.emit(PlayerEvent::StateChanged(new_state));
        }
    }

    /// Starts opening `path`, or stops if `path` is `None` (queue ran out).
    fn open_current_or_stop(&mut self, path: Option<PathBuf>) {
        match path {
            Some(path) => {
                let queue_index = self.queue.current_item_index().unwrap_or(0);
                self.start_open(path, queue_index);
            }
            None => self.set_state(PlaybackState::Stopped),
        }
    }

    /// Drops the current track (if any), stopping it and emitting the
    /// [`PlayerEvent::PlayFinished`] accounting summary for it.
    fn drop_current_and_account(&mut self) {
        if let Some(current) = self.current.take() {
            let _ = current.channel.stop();
            let completed = self.listen.completed(current.duration);
            self.emit(PlayerEvent::PlayFinished {
                path: current.path,
                listened: self.listen.listened(),
                completed,
            });
        }
        self.listen = ListenAccounting::new();
        self.last_tick = None;
    }

    /// Spawns a worker thread to open `path` off the calling (UI) thread —
    /// see the type-level docs for why. [`Player::tick`] picks up the
    /// result via [`Player::process_pending_open`].
    fn start_open(&mut self, path: PathBuf, queue_index: usize) {
        let (tx, rx) = bounded(1);
        let backend = Arc::clone(&self.backend);
        let end_tx = self.end_tx.clone();
        let spawned = thread::Builder::new()
            .name("emusic-player-open".to_string())
            .spawn(move || {
                let outcome = backend.open(&path).and_then(|channel| {
                    let guard = channel.on_end(Box::new(move || {
                        let _ = end_tx.send(());
                    }))?;
                    Ok((path, channel, guard))
                });
                let message = match outcome {
                    Ok((path, channel, guard)) => OpenMessage::Ready {
                        path,
                        queue_index,
                        channel,
                        guard,
                    },
                    Err(error) => OpenMessage::Failed { error },
                };
                let _ = tx.send(message);
            });
        match spawned {
            Ok(_join_handle) => self.pending_open = Some(rx),
            Err(io_error) => {
                self.emit(PlayerEvent::Error(PlayerError::SpawnFailed(
                    io_error.to_string(),
                )));
            }
        }
    }

    fn process_pending_open(&mut self) {
        let Some(rx) = &self.pending_open else {
            return;
        };
        match rx.try_recv() {
            Ok(OpenMessage::Ready {
                path,
                queue_index,
                channel,
                guard,
            }) => {
                self.pending_open = None;
                if let Err(error) = channel.set_volume(perceptual_to_gain(self.volume)) {
                    self.emit(PlayerEvent::Error(error));
                }
                if let Err(error) = channel.play(false) {
                    self.emit(PlayerEvent::Error(error));
                    return;
                }
                let duration = channel.duration().ok();
                self.current = Some(CurrentTrack {
                    channel,
                    _end_guard: guard,
                    path: path.clone(),
                    duration,
                });
                self.last_tick = Some(Instant::now());
                self.set_state(PlaybackState::Playing);
                self.emit(PlayerEvent::TrackStarted { path, queue_index });
            }
            Ok(OpenMessage::Failed { error }) => {
                self.pending_open = None;
                self.emit(PlayerEvent::Error(error));
                self.set_state(PlaybackState::Stopped);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.pending_open = None,
        }
    }

    fn process_end_signals(&mut self) {
        let mut ended = false;
        while self.end_rx.try_recv().is_ok() {
            ended = true;
        }
        if ended {
            self.handle_track_ended();
        }
    }

    fn handle_track_ended(&mut self) {
        let Some(path) = self.current.as_ref().map(|c| c.path.clone()) else {
            return;
        };
        self.accrue_listened_time();
        self.emit(PlayerEvent::TrackEnded { path: path.clone() });

        if self.queue.repeat_mode() == RepeatMode::One {
            let queue_index = self.queue.current_item_index().unwrap_or(0);
            self.drop_current_and_account();
            self.start_open(path, queue_index);
            return;
        }

        self.drop_current_and_account();
        let next = self.queue.advance();
        self.open_current_or_stop(next);
    }

    /// Adds wall-clock time since the last tick to listen accounting, but
    /// only while actually playing.
    fn accrue_listened_time(&mut self) {
        let now = Instant::now();
        if self.state == PlaybackState::Playing {
            if let Some(last) = self.last_tick {
                self.listen.add(now.duration_since(last));
            }
            self.last_tick = Some(now);
        } else {
            self.last_tick = None;
        }
    }
}
