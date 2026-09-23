//! Abstraction over the audio engine, so [`crate::Player`]'s queue/transport
//! logic can be unit-tested with a mock instead of a real BASS device.

use std::any::Any;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bass::{Attribute, Channel, FftSize, MusicFlags, StreamFlags};

use crate::error::PlayerError;
use crate::tracker::TrackerSettings;

/// Something that can open a playable channel for a file path.
///
/// Implemented by [`BassBackend`] for real playback; tests use a mock.
/// `Send + Sync` because [`crate::Player`] opens tracks on a worker thread
/// (see the module docs on [`crate::Player`] for why) while keeping a
/// shared handle to the backend.
pub trait AudioBackend: Send + Sync {
    /// Opens `path` as a playable channel, choosing a decoder based on the
    /// file extension (tracker modules vs. plain audio streams).
    fn open(&self, path: &Path) -> Result<Box<dyn BackendChannel>, PlayerError>;

    /// Sets the global `BASS_CONFIG_SRC` resampler quality (`0..=4`).
    fn set_tracker_resampling_quality(&self, quality: u8) -> Result<(), PlayerError>;
}

/// A single open, playable audio channel.
pub trait BackendChannel: Send {
    fn play(&self, restart: bool) -> Result<(), PlayerError>;
    fn pause(&self) -> Result<(), PlayerError>;
    fn stop(&self) -> Result<(), PlayerError>;
    /// Whether the channel is still playing/paused/stalled (not stopped).
    fn is_active(&self) -> bool;
    fn position(&self) -> Result<Duration, PlayerError>;
    fn duration(&self) -> Result<Duration, PlayerError>;
    fn seek(&self, position: Duration) -> Result<(), PlayerError>;
    /// Sets the channel's output gain, already curved (see
    /// [`crate::volume::perceptual_to_gain`]) — pass linear amplitude, not a
    /// raw UI slider value.
    fn set_volume(&self, gain: f32) -> Result<(), PlayerError>;
    /// Applies tracker-module-specific flags/attributes when `self` is a
    /// music channel. Plain stream channels ignore this.
    fn apply_tracker_settings(&self, settings: &TrackerSettings) -> Result<(), PlayerError>;
    /// Registers a callback that fires once when the channel reaches its
    /// end. The returned guard must be kept alive for as long as the
    /// callback should stay registered.
    fn on_end(&self, callback: Box<dyn Fn() + Send>) -> Result<Box<dyn Any + Send>, PlayerError>;

    /// Reads the channel's recent output as FFT magnitude bins (positive
    /// frequencies only), or `None` when the backend has no live channel /
    /// no FFT data to offer (e.g. a mock backend).
    fn fft(&self) -> Option<Vec<f32>> {
        None
    }

    /// Reads the channel's recent decoded float samples, or `None` when the
    /// backend has no live channel / can't supply samples.
    fn samples(&self) -> Option<Vec<f32>> {
        None
    }
}

/// File extensions BASS decodes as tracker modules (`BASS_MusicLoad`) rather
/// than plain streams (`BASS_StreamCreateFile`), per the issue's list.
const TRACKER_EXTENSIONS: &[&str] = &["mod", "s3m", "xm", "it", "mtm", "umx", "mo3"];

/// Whether `path`'s extension names a tracker module format.
pub fn is_tracker_module(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            TRACKER_EXTENSIONS
                .iter()
                .any(|t| t.eq_ignore_ascii_case(ext))
        })
}

/// Real playback backend, built on the `bass` crate.
pub struct BassBackend {
    bass: Arc<bass::Bass>,
}

impl BassBackend {
    /// Wraps an already-initialized [`bass::Bass`] instance, shared with the
    /// library scanner (which uses it to read tracker module tags). The
    /// caller owns `Bass::init`/plugin loading/device selection; this just
    /// opens channels through it.
    ///
    /// BASS's own documentation states the library is thread-safe (its
    /// functions may be called from any thread once initialized), which is
    /// what makes sharing one instance between the player and background
    /// scanner threads sound — see the `SAFETY` comment on
    /// `bass::ffi::BassLib`'s `Send`/`Sync` impls.
    pub fn new(bass: Arc<bass::Bass>) -> Self {
        Self { bass }
    }
}

impl AudioBackend for BassBackend {
    fn open(&self, path: &Path) -> Result<Box<dyn BackendChannel>, PlayerError> {
        // FLOAT decodes to `f32`, which the visualizer (#25) needs for its
        // oscilloscope; `get_data_fft` works regardless.
        let channel = if is_tracker_module(path) {
            BassChannel::Music(self.bass.open_music(path, MusicFlags::FLOAT, 0)?)
        } else {
            BassChannel::Stream(self.bass.open_stream(path, StreamFlags::FLOAT)?)
        };
        Ok(Box::new(channel))
    }

    fn set_tracker_resampling_quality(&self, quality: u8) -> Result<(), PlayerError> {
        self.bass
            .config()
            .set_resampling_quality(u32::from(quality))
            .map_err(PlayerError::Bass)
    }
}

/// Either kind of BASS channel we can play, unified behind
/// [`BackendChannel`].
enum BassChannel {
    Stream(bass::Stream),
    Music(bass::Music),
}

impl BassChannel {
    /// The underlying channel's format info via the [`bass::Channel`] trait,
    /// which isn't object-safe enough to call through an enum directly.
    fn channel_info(&self) -> Result<bass::ChannelInfo, bass::BassError> {
        match self {
            Self::Stream(s) => s.info(),
            Self::Music(m) => m.info(),
        }
    }
}

impl BackendChannel for BassChannel {
    fn play(&self, restart: bool) -> Result<(), PlayerError> {
        match self {
            Self::Stream(s) => s.play(restart)?,
            Self::Music(m) => m.play(restart)?,
        }
        Ok(())
    }

    fn pause(&self) -> Result<(), PlayerError> {
        match self {
            Self::Stream(s) => s.pause()?,
            Self::Music(m) => m.pause()?,
        }
        Ok(())
    }

    fn stop(&self) -> Result<(), PlayerError> {
        match self {
            Self::Stream(s) => s.stop()?,
            Self::Music(m) => m.stop()?,
        }
        Ok(())
    }

    fn is_active(&self) -> bool {
        match self {
            Self::Stream(s) => s.is_active(),
            Self::Music(m) => m.is_active(),
        }
    }

    fn position(&self) -> Result<Duration, PlayerError> {
        let seconds = match self {
            Self::Stream(s) => s.position_seconds()?,
            Self::Music(m) => m.position_seconds()?,
        };
        Ok(Duration::from_secs_f64(seconds.max(0.0)))
    }

    fn duration(&self) -> Result<Duration, PlayerError> {
        let seconds = match self {
            Self::Stream(s) => s.length_seconds()?,
            Self::Music(m) => m.length_seconds()?,
        };
        Ok(Duration::from_secs_f64(seconds.max(0.0)))
    }

    fn seek(&self, position: Duration) -> Result<(), PlayerError> {
        match self {
            Self::Stream(s) => s.seek(position.as_secs_f64())?,
            Self::Music(m) => m.seek(position.as_secs_f64())?,
        }
        Ok(())
    }

    fn set_volume(&self, gain: f32) -> Result<(), PlayerError> {
        match self {
            Self::Stream(s) => s.set_attribute(Attribute::Volume, gain)?,
            Self::Music(m) => m.set_attribute(Attribute::Volume, gain)?,
        }
        Ok(())
    }

    fn apply_tracker_settings(&self, settings: &TrackerSettings) -> Result<(), PlayerError> {
        match self {
            Self::Stream(_) => Ok(()),
            Self::Music(m) => settings.apply_to_music(m).map_err(PlayerError::Bass),
        }
    }

    fn on_end(&self, callback: Box<dyn Fn() + Send>) -> Result<Box<dyn Any + Send>, PlayerError> {
        let guard = match self {
            Self::Stream(s) => s.on_end(callback)?,
            Self::Music(m) => m.on_end(callback)?,
        };
        Ok(Box::new(guard))
    }

    fn fft(&self) -> Option<Vec<f32>> {
        let bins = match self {
            Self::Stream(s) => s.get_data_fft(FftSize::Fft1024),
            Self::Music(m) => m.get_data_fft(FftSize::Fft1024),
        };
        bins.ok()
    }

    fn samples(&self) -> Option<Vec<f32>> {
        // Read one interleaved FFT-sized frame and downmix to mono, so the
        // oscilloscope's single trace doesn't zig-zag between L and R.
        let mut raw = vec![0f32; FftSize::Fft1024.output_len() * MAX_SAMPLE_CHANNELS];
        let read = match self {
            Self::Stream(s) => s.get_data_f32(&mut raw),
            Self::Music(m) => m.get_data_f32(&mut raw),
        }
        .ok()?;
        if read == 0 {
            return None;
        }
        let channels = match self.channel_info() {
            Ok(info) => info.channels.clamp(1, MAX_SAMPLE_CHANNELS as u32) as usize,
            Err(_) => 1,
        };
        Some(downmix_mono(&raw[..read], channels))
    }
}

/// Upper bound on interleaved channels the oscilloscope buffer reserves for.
const MAX_SAMPLE_CHANNELS: usize = 2;

/// Averages each frame of `interleaved` (with `channels` samples per frame)
/// down to a single mono sample, so a stereo channel yields one clean trace.
fn downmix_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    let channels = channels.max(1);
    interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_extensions_are_recognized_case_insensitively() {
        for ext in ["mod", "S3M", "Xm", "it", "MTM", "umx", "mo3"] {
            assert!(is_tracker_module(Path::new(&format!("song.{ext}"))));
        }
    }

    #[test]
    fn plain_audio_extensions_are_not_tracker_modules() {
        for ext in ["mp3", "flac", "wav", "ogg"] {
            assert!(!is_tracker_module(Path::new(&format!("song.{ext}"))));
        }
    }

    #[test]
    fn no_extension_is_not_a_tracker_module() {
        assert!(!is_tracker_module(Path::new("no_extension")));
    }

    #[test]
    fn downmix_averages_each_stereo_frame() {
        let mono = downmix_mono(&[1.0, -1.0, 0.5, 0.5], 2);
        assert_eq!(mono, vec![0.0, 0.5]);
    }

    #[test]
    fn downmix_passes_mono_through() {
        let mono = downmix_mono(&[0.25, -0.5], 1);
        assert_eq!(mono, vec![0.25, -0.5]);
    }

    #[test]
    fn downmix_averages_a_partial_trailing_frame() {
        // A 3rd sample with no matching frame partner still contributes its
        // own (single-value) average rather than panicking.
        let mono = downmix_mono(&[1.0, 1.0, 0.0], 2);
        assert_eq!(mono, vec![1.0, 0.0]);
    }
}
