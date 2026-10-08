//! Where decoded samples go: a sound system that takes interleaved signed
//! 16-bit frames and says how many it has played.
//!
//! This module names nothing from emusic (it is the other half of the seam,
//! with [`crate::decode`]). A host implements [`Output`] over its sound system
//! (LazyOS: `audioclient::PlaybackStream` to `audiod`) and hands the player an
//! [`OutputFactory`]; [`MemoryOutput`] plays into memory against the clock,
//! for tests.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

/// Why the sound system refused a call.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("audio output: {0}")]
pub struct Error(pub String);

/// The output's result type.
pub type Result<T> = std::result::Result<T, Error>;

/// One open playback stream.
///
/// Frames go into a queue of limited size with [`try_write`](Self::try_write)
/// and play from it in order. The stream starts on its own once the queue is
/// full, or at [`start`](Self::start). Closing (dropping) it discards what has
/// not played. Used from one thread.
pub trait Output {
    /// The stream's frame rate.
    fn rate(&self) -> u32;
    /// Samples per frame.
    fn channels(&self) -> u16;
    /// The frames the sound system takes at a time: the last frames of a
    /// stream play only once they fill a whole period, so the caller pads the
    /// tail with silence.
    fn period_frames(&self) -> u64;
    /// Queues as many whole frames of interleaved `samples` as fit now;
    /// returns the frames queued.
    fn try_write(&mut self, samples: &[i16]) -> Result<usize>;
    /// Starts playing what is queued, full or not; does nothing when the
    /// stream is playing already.
    fn start(&mut self) -> Result<()>;
    /// Frames played since the stream opened.
    fn played(&mut self) -> Result<u64>;
    /// Scales the stream by `gain` (1.0 is unity).
    fn set_volume(&mut self, gain: f32) -> Result<()>;
}

/// Opens playback streams; shared with the player's threads.
pub trait OutputFactory: Send + Sync {
    /// Opens a stream of `channels`-sample frames at `rate` frames per second.
    fn open(&self, rate: u32, channels: u16) -> Result<Box<dyn Output>>;
}

/// What a [`MemorySink`] has heard, for tests.
#[derive(Debug, Default)]
pub struct Recording {
    /// Every sample played, interleaved, across all streams.
    pub samples: Vec<i16>,
    /// The parameters of each stream opened, in order.
    pub opened: Vec<(u32, u16)>,
    /// The gain of each stream at the moment each of its samples played,
    /// one entry per change: (sample index in `samples`, gain).
    pub gains: Vec<(usize, f32)>,
}

/// An [`OutputFactory`] whose streams play into one shared [`Recording`] in
/// (scaled) real time.
#[derive(Clone)]
pub struct MemorySink {
    recording: Arc<Mutex<Recording>>,
    queue_frames: u64,
    period_frames: u64,
    speed: f64,
}

impl MemorySink {
    /// A sink whose streams queue `queue_frames` frames, play whole periods of
    /// `period_frames`, and run `speed` times faster than real time.
    pub fn new(queue_frames: u64, period_frames: u64, speed: f64) -> MemorySink {
        MemorySink {
            recording: Arc::default(),
            queue_frames: queue_frames.max(1),
            period_frames: period_frames.clamp(1, queue_frames.max(1)),
            speed,
        }
    }

    /// What has played so far.
    pub fn recording(&self) -> MutexGuard<'_, Recording> {
        self.recording
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl OutputFactory for MemorySink {
    fn open(&self, rate: u32, channels: u16) -> Result<Box<dyn Output>> {
        if rate == 0 || channels == 0 {
            return Err(Error(format!("unsupported format {rate} Hz x{channels}")));
        }
        self.recording().opened.push((rate, channels));
        Ok(Box::new(MemoryOutput {
            sink: self.clone(),
            rate,
            channels,
            queue: VecDeque::new(),
            written: 0,
            played: 0,
            started_at: None,
            played_at_start: 0,
            gain: 1.0,
        }))
    }
}

/// A [`MemorySink`] stream: frames leave its queue for the recording, whole
/// periods at a time, as fast as the clock allows.
pub struct MemoryOutput {
    sink: MemorySink,
    rate: u32,
    channels: u16,
    queue: VecDeque<i16>,
    written: u64,
    played: u64,
    started_at: Option<Instant>,
    played_at_start: u64,
    gain: f32,
}

impl MemoryOutput {
    /// Moves the frames the clock says have played into the recording.
    fn advance(&mut self) {
        let Some(started_at) = self.started_at else {
            return;
        };
        let elapsed = started_at.elapsed().as_secs_f64() * self.sink.speed;
        let due = self.played_at_start + (elapsed * f64::from(self.rate)) as u64;
        // Like a period-based device, only whole periods play.
        let period = self.sink.period_frames;
        let playable = (self.written / period) * period;
        let target = due.min(playable);
        if target <= self.played {
            return;
        }
        let samples = ((target - self.played) * u64::from(self.channels)) as usize;
        let mut recording = self.sink.recording();
        let gain_changed = recording
            .gains
            .last()
            .is_none_or(|&(_, gain)| gain != self.gain);
        if gain_changed {
            let at = recording.samples.len();
            recording.gains.push((at, self.gain));
        }
        recording.samples.extend(self.queue.drain(..samples));
        self.played = target;
    }
}

impl Output for MemoryOutput {
    fn rate(&self) -> u32 {
        self.rate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn period_frames(&self) -> u64 {
        self.sink.period_frames
    }

    fn try_write(&mut self, samples: &[i16]) -> Result<usize> {
        let channels = usize::from(self.channels);
        if !samples.len().is_multiple_of(channels) {
            return Err(Error("a write that is not whole frames".into()));
        }
        self.advance();
        let free = (self.sink.queue_frames - (self.written - self.played)) as usize;
        let frames = free.min(samples.len() / channels);
        self.queue.extend(&samples[..frames * channels]);
        self.written += frames as u64;
        if self.started_at.is_none() && self.written - self.played >= self.sink.queue_frames {
            self.start()?;
        }
        Ok(frames)
    }

    fn start(&mut self) -> Result<()> {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
            self.played_at_start = self.played;
        }
        Ok(())
    }

    fn played(&mut self) -> Result<u64> {
        self.advance();
        Ok(self.played)
    }

    fn set_volume(&mut self, gain: f32) -> Result<()> {
        self.advance();
        self.gain = gain;
        Ok(())
    }
}
