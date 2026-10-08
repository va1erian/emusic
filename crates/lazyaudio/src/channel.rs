//! [`LazyChannel`]: emusic's [`BackendChannel`] over a decoder and an
//! [`Output`](crate::output::Output), driven by a worker thread.
//!
//! The only module of the crate besides [`crate::backend`] that knows emusic:
//! the decoder and the output know nothing of it.

use std::any::Any;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use emusic_player::backend::{BackendChannel, ChannelCapabilities, SeekSupport};
use emusic_player::error::PlayerError;
use emusic_player::tracker::TrackerSettings;

use crate::decode::{Mp3Decoder, SeekPrecision, StreamInfo, frames_to_duration};
use crate::output::OutputFactory;
use crate::scope::Scope;
use crate::worker::{Command, Request, Worker};

/// How long a command may wait for the worker before it fails.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// A registered end-of-track callback and its registration number.
type EndCallback = (u64, Box<dyn Fn() + Send>);

/// What the channel and its worker share.
pub(crate) struct Shared {
    /// The track frame that last played.
    position: AtomicU64,
    /// Playing or paused, not stopped or ended.
    active: AtomicBool,
    on_end: Mutex<Option<EndCallback>>,
    next_registration: AtomicU64,
    scope: Mutex<Scope>,
}

impl Shared {
    pub(crate) fn set_position(&self, frames: u64) {
        self.position.store(frames, Ordering::Release);
    }

    pub(crate) fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Release);
    }

    pub(crate) fn scope(&self) -> MutexGuard<'_, Scope> {
        self.scope
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn callbacks(&self) -> MutexGuard<'_, Option<EndCallback>> {
        self.on_end
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Runs the end-of-track callback, if one is registered.
    pub(crate) fn fire_end(&self) {
        if let Some((_, callback)) = self.callbacks().as_ref() {
            callback();
        }
    }
}

/// Unregisters an end-of-track callback when dropped.
struct EndGuard {
    shared: Arc<Shared>,
    registration: u64,
}

impl Drop for EndGuard {
    fn drop(&mut self) {
        let mut callbacks = self.shared.callbacks();
        if callbacks
            .as_ref()
            .is_some_and(|(id, _)| *id == self.registration)
        {
            *callbacks = None;
        }
    }
}

/// One open track.
pub struct LazyChannel {
    requests: Mutex<Sender<Request>>,
    shared: Arc<Shared>,
    info: StreamInfo,
}

impl LazyChannel {
    /// Starts the worker that plays `decoder` through streams `factory` opens.
    pub fn new(
        decoder: Mp3Decoder,
        factory: Arc<dyn OutputFactory>,
    ) -> Result<LazyChannel, PlayerError> {
        let info = *decoder.info();
        let shared = Arc::new(Shared {
            position: AtomicU64::new(0),
            active: AtomicBool::new(true),
            on_end: Mutex::new(None),
            next_registration: AtomicU64::new(0),
            scope: Mutex::new(Scope::default()),
        });
        let (requests, receiver) = mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        // The worker is built on its thread: the output streams it opens stay
        // there.
        std::thread::Builder::new()
            .name("emusic-lazyaudio".into())
            .spawn(move || Worker::new(decoder, factory, worker_shared).run(receiver))
            .map_err(|error| PlayerError::SpawnFailed(error.to_string()))?;
        Ok(LazyChannel {
            requests: Mutex::new(requests),
            shared,
            info,
        })
    }

    /// Sends `command` to the worker and waits for its outcome.
    fn send(&self, command: Command) -> Result<(), PlayerError> {
        let (reply, outcome) = mpsc::channel();
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .send(Request { command, reply })
            .map_err(|_| PlayerError::Backend("the playback thread has gone".into()))?;
        outcome
            .recv_timeout(COMMAND_TIMEOUT)
            .map_err(|_| PlayerError::Backend("the playback thread did not answer".into()))?
    }
}

impl BackendChannel for LazyChannel {
    fn play(&self, restart: bool) -> Result<(), PlayerError> {
        self.send(Command::Play { restart })
    }

    fn pause(&self) -> Result<(), PlayerError> {
        self.send(Command::Pause)
    }

    fn stop(&self) -> Result<(), PlayerError> {
        self.send(Command::Stop)
    }

    fn is_active(&self) -> bool {
        self.shared.active.load(Ordering::Acquire)
    }

    fn position(&self) -> Result<Duration, PlayerError> {
        let frames = self.shared.position.load(Ordering::Acquire);
        Ok(frames_to_duration(frames, self.info.rate))
    }

    fn duration(&self) -> Result<Duration, PlayerError> {
        Ok(self.info.duration.unwrap_or_default())
    }

    fn seek(&self, position: Duration) -> Result<(), PlayerError> {
        self.send(Command::Seek(position))
    }

    fn set_volume(&self, gain: f32) -> Result<(), PlayerError> {
        self.send(Command::Volume(gain))
    }

    fn apply_tracker_settings(&self, _settings: &TrackerSettings) -> Result<(), PlayerError> {
        Ok(())
    }

    fn on_end(&self, callback: Box<dyn Fn() + Send>) -> Result<Box<dyn Any + Send>, PlayerError> {
        let registration = self
            .shared
            .next_registration
            .fetch_add(1, Ordering::Relaxed);
        *self.shared.callbacks() = Some((registration, callback));
        Ok(Box::new(EndGuard {
            shared: Arc::clone(&self.shared),
            registration,
        }))
    }

    fn fft(&self) -> Option<Vec<f32>> {
        self.shared.scope().fft()
    }

    fn samples(&self) -> Option<Vec<f32>> {
        self.shared.scope().samples()
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            duration_known: self.info.duration.is_some(),
            seek: match self.info.seek {
                SeekPrecision::Exact => SeekSupport::Exact,
                SeekPrecision::Approximate => SeekSupport::Approximate,
            },
        }
    }
}
