#![forbid(unsafe_code)]

//! A playable SID tune: a BASS push stream fed by a background decoder thread.
//!
//! [`SidChannel`] opens the tune and starts a feeder thread that renders PCM
//! into a `bass::PushStream` (see #63). Rendering never happens on the UI
//! thread: the channel only forwards transport calls and, for seek-to-start,
//! sends a [`FeederCommand`] the feeder applies between render chunks.

use std::any::Any;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bass::{Attribute, Channel, FftSize, PushFlags, PushStream};
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};

use crate::backend::BackendChannel;
use crate::error::PlayerError;
use crate::tracker::TrackerSettings;

use super::decoder::{CrsidDecoder, SidDecoder};

/// Sample rate the engine renders at and the push stream is created with.
pub const SID_SAMPLE_RATE: u32 = 44_100;

/// Fixed tune length used until HVSC song lengths land (#65).
pub const DEFAULT_TUNE_LENGTH: Duration = Duration::from_secs(180);

/// Samples rendered per feeder iteration.
const RENDER_CHUNK: usize = 2_048;

/// How much PCM (in bytes) the feeder keeps queued ahead of playback: one
/// second of mono `f32` at [`SID_SAMPLE_RATE`]. Above this it idles, which is
/// also what throttles rendering while playback is paused.
const MAX_QUEUED_BYTES: u32 = SID_SAMPLE_RATE * 4;

/// How long the feeder sleeps when the queue is full.
const FEED_INTERVAL: Duration = Duration::from_millis(5);

/// Control messages the channel sends to its feeder thread.
enum FeederCommand {
    /// Reset the engine to the start of the current subtune and drop the
    /// already-queued PCM (seek-to-start).
    Restart,
    /// Stop rendering and let the feeder thread exit.
    Stop,
}

/// A SID tune played through a BASS push stream.
pub struct SidChannel {
    stream: Arc<PushStream>,
    commands: Sender<FeederCommand>,
}

impl SidChannel {
    /// Opens `path` as a SID tune and starts its feeder thread.
    ///
    /// Returns once the engine has loaded the tune, so load errors surface
    /// synchronously (on the caller's worker thread, never the UI thread).
    pub fn open(bass: &bass::Bass, path: &Path) -> Result<Self, PlayerError> {
        let data =
            std::fs::read(path).map_err(|error| PlayerError::ReadFailed(error.to_string()))?;

        let stream = Arc::new(bass.open_push_stream(SID_SAMPLE_RATE, 1, PushFlags::FLOAT)?);
        stream.set_duration(DEFAULT_TUNE_LENGTH.as_secs_f64());

        let (commands, command_rx) = unbounded();
        let (ready_tx, ready_rx) = bounded(1);
        let feeder_stream = Arc::clone(&stream);
        thread::Builder::new()
            .name("emusic-sid-feeder".to_string())
            .spawn(move || run_feeder(feeder_stream, data, command_rx, ready_tx))
            .map_err(|error| PlayerError::SpawnFailed(error.to_string()))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self { stream, commands }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(PlayerError::SpawnFailed(
                "SID feeder exited before loading the tune".to_string(),
            )),
        }
    }
}

impl BackendChannel for SidChannel {
    fn play(&self, restart: bool) -> Result<(), PlayerError> {
        if restart {
            let _ = self.commands.send(FeederCommand::Restart);
        }
        self.stream.play(false).map_err(PlayerError::Bass)
    }

    fn pause(&self) -> Result<(), PlayerError> {
        self.stream.pause().map_err(PlayerError::Bass)
    }

    fn stop(&self) -> Result<(), PlayerError> {
        let _ = self.commands.send(FeederCommand::Stop);
        self.stream.stop().map_err(PlayerError::Bass)
    }

    fn is_active(&self) -> bool {
        self.stream.is_active()
    }

    fn position(&self) -> Result<Duration, PlayerError> {
        Ok(Duration::from_secs_f64(
            self.stream.position_seconds()?.max(0.0),
        ))
    }

    fn duration(&self) -> Result<Duration, PlayerError> {
        Ok(Duration::from_secs_f64(
            self.stream.length_seconds()?.max(0.0),
        ))
    }

    /// SID tunes can't be seeked within; only a seek to the start is honoured,
    /// by resetting the engine and clearing the queued PCM.
    fn seek(&self, position: Duration) -> Result<(), PlayerError> {
        if position.is_zero() {
            let _ = self.commands.send(FeederCommand::Restart);
        }
        Ok(())
    }

    fn set_volume(&self, gain: f32) -> Result<(), PlayerError> {
        self.stream
            .set_attribute(Attribute::Volume, gain)
            .map_err(PlayerError::Bass)
    }

    fn apply_tracker_settings(&self, _settings: &TrackerSettings) -> Result<(), PlayerError> {
        Ok(())
    }

    fn on_end(&self, callback: Box<dyn Fn() + Send>) -> Result<Box<dyn Any + Send>, PlayerError> {
        let guard = self.stream.on_end(callback)?;
        Ok(Box::new(guard))
    }

    fn fft(&self) -> Option<Vec<f32>> {
        self.stream.get_data_fft(FftSize::Fft1024).ok()
    }

    fn samples(&self) -> Option<Vec<f32>> {
        let mut raw = vec![0f32; FftSize::Fft1024.output_len()];
        let read = self.stream.get_data_f32(&mut raw).ok()?;
        if read == 0 {
            return None;
        }
        Some(raw[..read].to_vec())
    }
}

impl Drop for SidChannel {
    fn drop(&mut self) {
        let _ = self.commands.send(FeederCommand::Stop);
    }
}

/// The feeder thread: loads the tune, reports the result through `ready`, then
/// renders into `stream` until it ends or is told to stop.
fn run_feeder(
    stream: Arc<PushStream>,
    data: Vec<u8>,
    commands: Receiver<FeederCommand>,
    ready: Sender<Result<(), PlayerError>>,
) {
    let mut decoder = match CrsidDecoder::from_bytes(data, SID_SAMPLE_RATE, Default::default()) {
        Ok(decoder) => {
            let _ = ready.send(Ok(()));
            decoder
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };

    let total_frames = (DEFAULT_TUNE_LENGTH.as_secs_f64() * f64::from(SID_SAMPLE_RATE)) as u64;
    let mut remaining = total_frames;
    let mut pcm = vec![0i16; RENDER_CHUNK];
    let mut bytes = Vec::with_capacity(RENDER_CHUNK * size_of::<f32>());

    loop {
        match commands.try_recv() {
            Ok(FeederCommand::Restart) => {
                if decoder.restart().is_err() {
                    let _ = stream.end_of_stream();
                    return;
                }
                remaining = total_frames;
                // Clears the push stream's queue and resets its position.
                let _ = stream.set_position_bytes(0);
            }
            Ok(FeederCommand::Stop) | Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {}
        }

        if remaining == 0 {
            // Let BASS play out the queue and fire the end sync.
            let _ = stream.end_of_stream();
            return;
        }

        if stream.queued_bytes().unwrap_or(0) > MAX_QUEUED_BYTES {
            thread::sleep(FEED_INTERVAL);
            continue;
        }

        let frames = RENDER_CHUNK.min(remaining as usize);
        if decoder.render(&mut pcm[..frames]).is_err() {
            let _ = stream.end_of_stream();
            return;
        }

        bytes.clear();
        for &sample in &pcm[..frames] {
            bytes.extend_from_slice(&(f32::from(sample) / 32_768.0).to_le_bytes());
        }
        if stream.push_data(&bytes).is_err() {
            return;
        }
        remaining -= frames as u64;
    }
}
