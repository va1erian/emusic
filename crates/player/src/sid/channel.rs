#![forbid(unsafe_code)]

//! A playable SID tune: a BASS push stream fed by a background decoder thread.
//!
//! [`SidChannel`] opens the tune and starts a feeder thread that renders PCM
//! into a `bass::PushStream` (see #63). Rendering never happens on the UI
//! thread: the channel only forwards transport calls and subtune changes as
//! [`FeederCommand`]s the feeder applies between render chunks.

use std::any::Any;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::thread;
use std::time::Duration;

use bass::{Attribute, Channel, FftSize, PushFlags, PushStream};
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};
use emusic_sid::HvscIndex;

use crate::backend::BackendChannel;
use crate::error::PlayerError;
use crate::tracker::TrackerSettings;

use super::decoder::{CrsidDecoder, SidDecoder};
use super::info::SidInfo;
use super::settings::SidSettings;

/// Sample rate the engine renders at and the push stream is created with.
pub const SID_SAMPLE_RATE: u32 = 44_100;

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
    /// Switch to another subtune and restart it.
    SelectSubtune(u16),
    /// Stop rendering and let the feeder thread exit.
    Stop,
}

/// A SID tune played through a BASS push stream.
pub struct SidChannel {
    stream: Arc<PushStream>,
    commands: Sender<FeederCommand>,
    /// Static metadata plus the default subtune; the live subtune is read from
    /// `current` in [`BackendChannel::sid_info`].
    info: SidInfo,
    /// The subtune the feeder is currently playing.
    current: Arc<AtomicU16>,
}

impl SidChannel {
    /// Opens `path` as a SID tune and starts its feeder thread.
    ///
    /// `settings` supplies the chip/clock overrides and the fallback length;
    /// `hvsc`, when set, supplies per-subtune lengths. Returns once the engine
    /// has loaded the tune, so load errors surface synchronously (on the
    /// caller's worker thread, never the UI thread).
    pub fn open(
        bass: &bass::Bass,
        path: &Path,
        settings: SidSettings,
        hvsc: Option<Arc<HvscIndex>>,
    ) -> Result<Self, PlayerError> {
        let data =
            std::fs::read(path).map_err(|error| PlayerError::ReadFailed(error.to_string()))?;
        let header = emusic_sid::SidHeader::parse(&data)?;

        let lengths = hvsc
            .as_ref()
            .and_then(|index| index.lengths(&data))
            .map(<[Duration]>::to_vec);
        let fallback = settings.default_length();
        let subtune = header.default_subtune;
        let initial_length = length_for(subtune, lengths.as_deref(), fallback);

        let stream = Arc::new(bass.open_push_stream(SID_SAMPLE_RATE, 1, PushFlags::FLOAT)?);
        stream.set_duration(initial_length.as_secs_f64());

        let current = Arc::new(AtomicU16::new(subtune));
        let info = SidInfo::from_header(&header, settings, subtune);

        let (commands, command_rx) = unbounded();
        let (ready_tx, ready_rx) = bounded(1);
        let feeder_stream = Arc::clone(&stream);
        let feeder_current = Arc::clone(&current);
        let engine_config = settings.to_engine_config();
        thread::Builder::new()
            .name("emusic-sid-feeder".to_string())
            .spawn(move || {
                run_feeder(
                    feeder_stream,
                    data,
                    engine_config,
                    lengths,
                    fallback,
                    feeder_current,
                    command_rx,
                    ready_tx,
                )
            })
            .map_err(|error| PlayerError::SpawnFailed(error.to_string()))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stream,
                commands,
                info,
                current,
            }),
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

    fn sid_info(&self) -> Option<SidInfo> {
        let mut info = self.info.clone();
        info.current_subtune = self.current.load(Ordering::Relaxed);
        Some(info)
    }

    fn select_subtune(&self, subtune: u16) -> Result<(), PlayerError> {
        let _ = self.commands.send(FeederCommand::SelectSubtune(subtune));
        Ok(())
    }
}

impl Drop for SidChannel {
    fn drop(&mut self) {
        let _ = self.commands.send(FeederCommand::Stop);
    }
}

/// The feeder thread: loads the tune, reports the result through `ready`, then
/// renders into `stream` until it ends or is told to stop.
#[allow(clippy::too_many_arguments)]
fn run_feeder(
    stream: Arc<PushStream>,
    data: Vec<u8>,
    engine_config: emusic_sid::SidConfig,
    lengths: Option<Vec<Duration>>,
    fallback: Duration,
    current: Arc<AtomicU16>,
    commands: Receiver<FeederCommand>,
    ready: Sender<Result<(), PlayerError>>,
) {
    let mut decoder = match CrsidDecoder::from_bytes(data, SID_SAMPLE_RATE, engine_config) {
        Ok(decoder) => {
            let _ = ready.send(Ok(()));
            decoder
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };

    let mut subtune = decoder.current_subtune();
    current.store(subtune, Ordering::Relaxed);
    let mut remaining = frames_of(length_for(subtune, lengths.as_deref(), fallback));

    let mut pcm = vec![0i16; RENDER_CHUNK];
    let mut bytes = Vec::with_capacity(RENDER_CHUNK * size_of::<f32>());

    loop {
        match commands.try_recv() {
            Ok(FeederCommand::Restart) => {
                if decoder.restart().is_err() {
                    let _ = stream.end_of_stream();
                    return;
                }
                remaining = frames_of(length_for(subtune, lengths.as_deref(), fallback));
                // Clears the push stream's queue and resets its position.
                let _ = stream.set_position_bytes(0);
            }
            Ok(FeederCommand::SelectSubtune(requested)) => {
                if decoder.select_subtune(requested).is_ok() {
                    subtune = decoder.current_subtune();
                    current.store(subtune, Ordering::Relaxed);
                    let length = length_for(subtune, lengths.as_deref(), fallback);
                    stream.set_duration(length.as_secs_f64());
                    remaining = frames_of(length);
                    let _ = stream.set_position_bytes(0);
                }
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

/// The length to use for `subtune`: the HVSC value if known, else `fallback`.
fn length_for(subtune: u16, lengths: Option<&[Duration]>, fallback: Duration) -> Duration {
    lengths
        .and_then(|lengths| lengths.get(usize::from(subtune.saturating_sub(1))).copied())
        .unwrap_or(fallback)
}

/// Converts a duration to a frame count at [`SID_SAMPLE_RATE`].
fn frames_of(length: Duration) -> u64 {
    (length.as_secs_f64() * f64::from(SID_SAMPLE_RATE)) as u64
}
