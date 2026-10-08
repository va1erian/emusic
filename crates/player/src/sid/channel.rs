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
use std::sync::atomic::{AtomicU16, Ordering};
use std::thread;
use std::time::Duration;

use bass::{Attribute, Channel, FftSize, PushFlags, PushStream};
use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, unbounded};
use emusic_sid::{SidHeader, SongLengths};

use crate::backend::{BackendChannel, ChannelCapabilities, SeekSupport, Subsong};
use crate::error::PlayerError;
use crate::tracker::TrackerSettings;

use super::decoder::{CrsidDecoder, SidDecoder};

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
    /// Switch to this subtune (`1`-based, already clamped) and restart it,
    /// dropping the already-queued PCM.
    SelectSubtune(u16),
    /// Stop rendering and let the feeder thread exit.
    Stop,
}

/// A SID tune played through a BASS push stream.
pub struct SidChannel {
    stream: Arc<PushStream>,
    commands: Sender<FeederCommand>,
    /// Each subtune's real length from the database (index 0 = subtune 1,
    /// `None` when unknown); shared with the feeder and read by
    /// [`SidChannel::capabilities`] for the currently loaded subtune.
    subtune_lengths: Arc<Vec<Option<Duration>>>,
    /// How many subtunes the tune contains (`>= 1`).
    subtune_count: u16,
    /// The subtune currently loaded, shared with the feeder so a switch is
    /// reflected here as soon as the engine applies it.
    current_subtune: Arc<AtomicU16>,
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
        let header = SidHeader::parse(&data)?;

        // The database, if any, is keyed by the whole file and lists every
        // subtune's length. Resolve them all up front so the feeder can switch
        // subtunes without touching the database (which lives on the UI side).
        let subtune_lengths = Arc::new(resolve_subtune_lengths(&data, header.subtunes, lengths));
        let length = subtune_lengths[usize::from(header.default_subtune - 1)];

        let stream = Arc::new(bass.open_push_stream(SID_SAMPLE_RATE, 1, PushFlags::FLOAT)?);
        if let Some(length) = length {
            stream.set_duration(length.as_secs_f64());
        }

        let current_subtune = Arc::new(AtomicU16::new(header.default_subtune));
        let (commands, command_rx) = unbounded();
        let (ready_tx, ready_rx) = bounded(1);
        let feeder_stream = Arc::clone(&stream);
        let feeder_current = Arc::clone(&current_subtune);
        let feeder_lengths = Arc::clone(&subtune_lengths);
        thread::Builder::new()
            .name("emusic-sid-feeder".to_string())
            .spawn(move || {
                run_feeder(
                    feeder_stream,
                    data,
                    command_rx,
                    ready_tx,
                    SidFeeder::new(feeder_lengths, fallback),
                    feeder_current,
                );
            })
            .map_err(|error| PlayerError::SpawnFailed(error.to_string()))?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                stream,
                commands,
                subtune_lengths,
                subtune_count: header.subtunes,
                current_subtune,
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(PlayerError::SpawnFailed(
                "SID feeder exited before loading the tune".to_string(),
            )),
        }
    }
}

/// Resolves each subtune's length from `lengths` (index `0` = subtune 1),
/// `None` where there is no database or no entry for that subtune.
fn resolve_subtune_lengths(
    data: &[u8],
    subtunes: u16,
    lengths: Option<&SongLengths>,
) -> Vec<Option<Duration>> {
    // Hash the tune once, then index the resolved list per subtune.
    let entries = lengths.and_then(|db| db.subtunes(data));
    (1..=subtunes)
        .map(|n| {
            entries
                .and_then(|entries| entries.get(usize::from(n - 1)))
                .copied()
        })
        .collect()
}

/// Per-subtune play-length bookkeeping for the feeder: the resolved list (one
/// entry per subtune, `None` when the database doesn't cover it) and the
/// fallback used for those.
struct SidFeeder {
    subtune_lengths: Arc<Vec<Option<Duration>>>,
    fallback: Duration,
}

impl SidFeeder {
    fn new(subtune_lengths: Arc<Vec<Option<Duration>>>, fallback: Duration) -> Self {
        debug_assert!(
            !subtune_lengths.is_empty(),
            "a parsed SID header always has at least one subtune"
        );
        Self {
            subtune_lengths,
            fallback,
        }
    }

    /// The length `subtune` (`1`-based) actually plays for: its database entry
    /// when known, otherwise the fallback so playback still stops.
    fn play_length(&self, subtune: u16) -> Duration {
        self.subtune_lengths
            .get(usize::from(subtune.saturating_sub(1)))
            .copied()
            .flatten()
            .unwrap_or(self.fallback)
    }

    /// The known length for `subtune`, or `None` when the database doesn't
    /// cover it.
    fn known_length(&self, subtune: u16) -> Option<Duration> {
        self.subtune_lengths
            .get(usize::from(subtune.saturating_sub(1)))
            .copied()
            .flatten()
    }
}

/// The number of sample frames in `length` at [`SID_SAMPLE_RATE`].
fn frames(length: Duration) -> u64 {
    (length.as_secs_f64() * f64::from(SID_SAMPLE_RATE)) as u64
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

    /// A SID file carries one or more subtunes; report the current one and
    /// the count so the now-playing panel can offer a selector.
    fn subsong(&self) -> Option<Subsong> {
        Some(Subsong {
            current: self.current_subtune.load(Ordering::Relaxed),
            count: self.subtune_count,
        })
    }

    /// Switches to `subtune` (`1`-based, clamped to the valid range); the
    /// feeder restarts the engine at its init routine and resets the queue.
    ///
    /// The feeder is the only writer of the current subtune, so this does not
    /// update it optimistically: `subsong()` only reflects a switch once the
    /// engine has actually applied it.
    fn select_subsong(&self, subtune: u16) -> Result<(), PlayerError> {
        let subtune = subtune.clamp(1, self.subtune_count);
        let _ = self.commands.send(FeederCommand::SelectSubtune(subtune));
        Ok(())
    }

    /// SID tunes can't be seeked within (only a restart to zero is honoured),
    /// so the slider is disabled rather than silently ignored. Whether the
    /// length is known depends on the *current* subtune's HVSC entry (#192),
    /// so it is re-evaluated here rather than fixed at load.
    fn capabilities(&self) -> ChannelCapabilities {
        let current = usize::from(
            self.current_subtune
                .load(Ordering::Relaxed)
                .saturating_sub(1),
        );
        let duration_known = self
            .subtune_lengths
            .get(current)
            .copied()
            .flatten()
            .is_some();
        ChannelCapabilities {
            duration_known,
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
    feeder: SidFeeder,
    current_subtune: Arc<AtomicU16>,
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

    let mut total_frames = frames(feeder.play_length(current_subtune.load(Ordering::Relaxed)));
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
                // Clears the push stream's queue and resets the position.
                let _ = stream.set_position_bytes(0);
            }
            Ok(FeederCommand::SelectSubtune(next)) => {
                if decoder.select_subtune(next).is_err() {
                    let _ = stream.end_of_stream();
                    return;
                }
                current_subtune.store(next, Ordering::Relaxed);
                if let Some(length) = feeder.known_length(next) {
                    stream.set_duration(length.as_secs_f64());
                }
                total_frames = frames(feeder.play_length(next));
                remaining = total_frames;
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

        let count = RENDER_CHUNK.min(remaining as usize);
        if decoder.render(&mut pcm[..count]).is_err() {
            let _ = stream.end_of_stream();
            return;
        }

        bytes.clear();
        for &sample in &pcm[..count] {
            bytes.extend_from_slice(&(f32::from(sample) / 32_768.0).to_le_bytes());
        }
        if stream.push_data(&bytes).is_err() {
            return;
        }
        remaining -= count as u64;
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
    fn resolve_subtune_lengths_reads_every_subtune_entry() {
        let data = psid(2);
        let db = database(&data, "1:00 2:30 3:00");
        assert_eq!(
            resolve_subtune_lengths(&data, 3, Some(&db)),
            vec![
                Some(Duration::from_secs(60)),
                Some(Duration::from_secs(150)),
                Some(Duration::from_secs(180)),
            ]
        );
    }

    #[test]
    fn resolve_subtune_lengths_is_none_without_a_database_or_entry() {
        let data = psid(1);
        assert_eq!(resolve_subtune_lengths(&data, 3, None), vec![None; 3]);
        let other = database(b"a different tune", "1:00");
        assert_eq!(
            resolve_subtune_lengths(&data, 3, Some(&other)),
            vec![None; 3]
        );
    }

    #[test]
    fn the_feeder_prefers_a_known_length_and_falls_back_otherwise() {
        let feeder = SidFeeder::new(
            Arc::new(vec![Some(Duration::from_secs(60)), None]),
            Duration::from_secs(180),
        );
        assert_eq!(feeder.play_length(1), Duration::from_secs(60));
        assert_eq!(feeder.play_length(2), Duration::from_secs(180));
        assert_eq!(feeder.known_length(1), Some(Duration::from_secs(60)));
        assert_eq!(feeder.known_length(2), None);
    }
}
