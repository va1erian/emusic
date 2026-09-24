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
use emusic_sid::{SidHeader, SongLengths};

use crate::backend::{BackendChannel, ChannelCapabilities, SeekSupport};
use crate::error::PlayerError;
use crate::tracker::TrackerSettings;

use super::decoder::{CrsidDecoder, SidDecoder};

/// Sample rate the engine renders at and the push stream is created with.
pub const SID_SAMPLE_RATE: u32 = 44_100;

/// Fallback play length for a SID tune with no HVSC Songlengths entry (#192):
/// long enough to hear the tune, short enough that playback doesn't sit on an
/// unknown-length track forever. The real per-subtune length is used whenever
/// the database has one.
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
    /// Whether the tune's real length (from the database) is known; drives
    /// [`ChannelCapabilities::duration_known`].
    duration_known: bool,
}

impl SidChannel {
    /// Opens `path` as a SID tune and starts its feeder thread.
    ///
    /// Returns once the engine has loaded the tune, so load errors surface
    /// synchronously (on the caller's worker thread, never the UI thread).
    ///
    /// `lengths` is the user's HVSC Songlengths database, if any: when it
    /// holds the current subtune, that length is reported and used to stop
    /// playback; otherwise `fallback` is played (and the length is reported as
    /// unknown) so the tune still ends and the queue advances (#192).
    pub fn open(
        bass: &bass::Bass,
        path: &Path,
        lengths: Option<&SongLengths>,
        fallback: Duration,
    ) -> Result<Self, PlayerError> {
        let data =
            std::fs::read(path).map_err(|error| PlayerError::ReadFailed(error.to_string()))?;

        let stream = Arc::new(bass.open_push_stream(SID_SAMPLE_RATE, 1, PushFlags::FLOAT)?);
        let length = tune_length(&data, lengths);
        if let Some(length) = length {
            stream.set_duration(length.as_secs_f64());
        }
        let play_length = length.unwrap_or(fallback);

        let (commands, command_rx) = unbounded();
        let (ready_tx, ready_rx) = bounded(1);
        let feeder_stream = Arc::clone(&stream);
        thread::Builder::new()
            .name("emusic-sid-feeder".to_string())
            .spawn(move || run_feeder(feeder_stream, data, command_rx, ready_tx, play_length))
            .map_err(|error| PlayerError::SpawnFailed(error.to_string()))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stream,
                commands,
                duration_known: length.is_some(),
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(PlayerError::SpawnFailed(
                "SID feeder exited before loading the tune".to_string(),
            )),
        }
    }
}

/// The real length of `data`'s default subtune from `lengths`, or `None` when
/// there is no database or the tune/subtune isn't in it.
fn tune_length(data: &[u8], lengths: Option<&SongLengths>) -> Option<Duration> {
    let lengths = lengths?;
    let header = SidHeader::parse(data).ok()?;
    lengths.subtune(data, header.default_subtune)
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

    /// SID tunes can't be seeked within (only a restart to zero is honoured),
    /// so the slider is disabled rather than silently ignored. The length is
    /// only known when the HVSC database had the current subtune (#192).
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            duration_known: self.duration_known,
            seek: SeekSupport::Unsupported,
        }
    }
}

impl Drop for SidChannel {
    fn drop(&mut self) {
        let _ = self.commands.send(FeederCommand::Stop);
    }
}

/// The feeder thread: loads the tune, reports the result through `ready`, then
/// renders into `stream` until the tune's `play_length` is reached, it is told
/// to stop, or it errors.
fn run_feeder(
    stream: Arc<PushStream>,
    data: Vec<u8>,
    commands: Receiver<FeederCommand>,
    ready: Sender<Result<(), PlayerError>>,
    play_length: Duration,
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

    let total_frames = (play_length.as_secs_f64() * f64::from(SID_SAMPLE_RATE)) as u64;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal valid PSID v1 header with the given default subtune.
    fn psid(default_subtune: u16) -> Vec<u8> {
        let mut data = vec![0u8; 0x76];
        data[0..4].copy_from_slice(b"PSID");
        data[0x05] = 1;
        data[0x06..0x08].copy_from_slice(&0x76u16.to_be_bytes());
        data[0x0E..0x10].copy_from_slice(&3u16.to_be_bytes());
        data[0x10..0x12].copy_from_slice(&default_subtune.to_be_bytes());
        data
    }

    /// A one-entry database keyed for `data`.
    fn database(data: &[u8], lengths: &str) -> SongLengths {
        let key: String = SongLengths::md5(data)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        SongLengths::parse(&format!("[Database]\n{key}={lengths}\n"))
    }

    #[test]
    fn tune_length_uses_the_default_subtune_entry() {
        let data = psid(2);
        let db = database(&data, "1:00 2:30 3:00");
        assert_eq!(
            tune_length(&data, Some(&db)),
            Some(Duration::from_secs(150))
        );
    }

    #[test]
    fn tune_length_is_none_without_a_database_or_entry() {
        let data = psid(1);
        assert_eq!(tune_length(&data, None), None);
        let other = database(b"a different tune", "1:00");
        assert_eq!(tune_length(&data, Some(&other)), None);
    }
}
