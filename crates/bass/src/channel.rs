//! The [`Channel`] trait: operations common to [`crate::Stream`] and
//! [`crate::Music`] handles.

use std::sync::Arc;

use crate::error::BassError;
use crate::ffi::BassLib;
use crate::ffi::types::Dword;
use crate::flags::{Attribute, FftSize, PlaybackState, PositionMode};
use crate::sync::ChannelSync;

/// Info about a channel's audio format, from `BASS_ChannelGetInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelInfo {
    /// Sample rate, in Hz.
    pub freq: u32,
    /// Number of channels (1 = mono, 2 = stereo, ...).
    pub channels: u32,
    /// Coarse, human-readable description of the channel type, derived
    /// from BASS's `ctype` value (e.g. `"Stream"`, `"Music"`).
    pub format_name: String,
}

/// Operations shared by every playable BASS channel handle
/// ([`crate::Stream`], [`crate::Music`]).
///
/// This trait can't be implemented outside this crate: [`Channel::lib`]
/// returns a crate-private type, so external code has no way to write a
/// conforming `impl`.
pub trait Channel {
    /// The raw BASS channel handle (`HSTREAM`/`HMUSIC`). Exposed for
    /// advanced use; most callers won't need it.
    fn handle(&self) -> Dword;

    /// Not part of the public API: exists only so default methods below
    /// can reach the loaded library. External crates cannot implement this
    /// (the return type is private), which is what keeps [`Channel`]
    /// effectively sealed.
    #[doc(hidden)]
    fn lib(&self) -> &Arc<BassLib>;

    /// Starts (or resumes) playback.
    ///
    /// `restart` seeks back to the start of the channel first.
    fn play(&self, restart: bool) -> Result<(), BassError> {
        // SAFETY: `self.handle()` names a channel owned by `self`, valid
        // for the duration of this call.
        let ok = unsafe { (self.lib().raw.bass_channel_play)(self.handle(), restart as _) } != 0;
        self.lib().check(ok)
    }

    /// Pauses playback.
    fn pause(&self) -> Result<(), BassError> {
        // SAFETY: see `play`.
        let ok = unsafe { (self.lib().raw.bass_channel_pause)(self.handle()) } != 0;
        self.lib().check(ok)
    }

    /// Stops playback (resets the position, unlike [`Channel::pause`]).
    fn stop(&self) -> Result<(), BassError> {
        // SAFETY: see `play`.
        let ok = unsafe { (self.lib().raw.bass_channel_stop)(self.handle()) } != 0;
        self.lib().check(ok)
    }

    /// The channel's current playback state.
    fn state(&self) -> PlaybackState {
        // SAFETY: see `play`.
        let raw = unsafe { (self.lib().raw.bass_channel_is_active)(self.handle()) };
        PlaybackState::from_raw(raw)
    }

    /// Whether the channel is playing, stalled or paused (i.e. not fully
    /// stopped).
    fn is_active(&self) -> bool {
        self.state() != PlaybackState::Stopped
    }

    /// The channel's total length, in the given [`PositionMode`]'s units.
    fn length(&self, mode: PositionMode) -> Result<u64, BassError> {
        // SAFETY: see `play`.
        let value =
            unsafe { (self.lib().raw.bass_channel_get_length)(self.handle(), mode.as_raw()) };
        if value == u64::MAX {
            Err(self.lib().last_error())
        } else {
            Ok(value)
        }
    }

    /// The channel's total length in seconds.
    fn length_seconds(&self) -> Result<f64, BassError> {
        let bytes = self.length(PositionMode::Bytes)?;
        self.bytes_to_seconds(bytes)
    }

    /// The channel's current playback position, in the given
    /// [`PositionMode`]'s units.
    fn position(&self, mode: PositionMode) -> Result<u64, BassError> {
        // SAFETY: see `play`.
        let value =
            unsafe { (self.lib().raw.bass_channel_get_position)(self.handle(), mode.as_raw()) };
        if value == u64::MAX {
            Err(self.lib().last_error())
        } else {
            Ok(value)
        }
    }

    /// The channel's current playback position, in seconds.
    fn position_seconds(&self) -> Result<f64, BassError> {
        let bytes = self.position(PositionMode::Bytes)?;
        self.bytes_to_seconds(bytes)
    }

    /// Seeks to `seconds` from the start of the channel.
    fn seek(&self, seconds: f64) -> Result<(), BassError> {
        // SAFETY: see `play`.
        let bytes =
            unsafe { (self.lib().raw.bass_channel_seconds_2_bytes)(self.handle(), seconds) };
        if bytes == u64::MAX {
            return Err(self.lib().last_error());
        }
        self.set_position_bytes(bytes)
    }

    /// Seeks to an exact byte position.
    fn set_position_bytes(&self, bytes: u64) -> Result<(), BassError> {
        // SAFETY: see `play`.
        let ok = unsafe {
            (self.lib().raw.bass_channel_set_position)(
                self.handle(),
                bytes,
                PositionMode::Bytes.as_raw(),
            )
        } != 0;
        self.lib().check(ok)
    }

    /// Converts a byte position to seconds, for this channel's format.
    fn bytes_to_seconds(&self, bytes: u64) -> Result<f64, BassError> {
        // SAFETY: see `play`.
        let seconds =
            unsafe { (self.lib().raw.bass_channel_bytes_2_seconds)(self.handle(), bytes) };
        if seconds < 0.0 {
            Err(self.lib().last_error())
        } else {
            Ok(seconds)
        }
    }

    /// Reads a channel attribute (volume, pan, tracker amplify/pan-sep, ...).
    fn attribute(&self, attribute: Attribute) -> Result<f32, BassError> {
        let mut value: f32 = 0.0;
        // SAFETY: `value` is a valid, writable `f32` for the duration of
        // this call.
        let ok = unsafe {
            (self.lib().raw.bass_channel_get_attribute)(
                self.handle(),
                attribute.as_raw(),
                &mut value,
            )
        } != 0;
        self.lib().check(ok)?;
        Ok(value)
    }

    /// Sets a channel attribute (volume, pan, tracker amplify/pan-sep, ...).
    fn set_attribute(&self, attribute: Attribute, value: f32) -> Result<(), BassError> {
        // SAFETY: see `play`.
        let ok = unsafe {
            (self.lib().raw.bass_channel_set_attribute)(self.handle(), attribute.as_raw(), value)
        } != 0;
        self.lib().check(ok)
    }

    /// Modifies the channel's `BASS_ChannelFlags`: bits set in `mask` are
    /// replaced with the corresponding bits from `flags`; other bits are
    /// left untouched. Returns the resulting full flag set.
    fn set_flags(&self, flags: u32, mask: u32) -> Result<u32, BassError> {
        // SAFETY: see `play`.
        let result = unsafe { (self.lib().raw.bass_channel_flags)(self.handle(), flags, mask) };
        if result == Dword::MAX {
            Err(self.lib().last_error())
        } else {
            Ok(result)
        }
    }

    /// Format information for this channel (sample rate, channel count,
    /// coarse type description).
    fn info(&self) -> Result<ChannelInfo, BassError> {
        let mut raw = crate::ffi::types::BassChannelInfo::default();
        // SAFETY: `raw` is a valid, writable `BASS_CHANNELINFO` for the
        // duration of this call.
        let ok = unsafe { (self.lib().raw.bass_channel_get_info)(self.handle(), &mut raw) } != 0;
        self.lib().check(ok)?;
        Ok(ChannelInfo {
            freq: raw.freq,
            channels: raw.chans,
            format_name: format_name(raw.ctype),
        })
    }

    /// Runs an FFT over the channel's recent output and returns the
    /// magnitude spectrum (`size.output_len()` values, positive
    /// frequencies only).
    fn get_data_fft(&self, size: FftSize) -> Result<Vec<f32>, BassError> {
        let mut buffer = vec![0f32; size.output_len()];
        // SAFETY: `buffer` is writable for `buffer.len() * 4` bytes, which
        // matches what BASS writes for this FFT size.
        let result = unsafe {
            (self.lib().raw.bass_channel_get_data)(
                self.handle(),
                buffer.as_mut_ptr().cast(),
                size.as_raw(),
            )
        };
        if result < 0 {
            return Err(self.lib().last_error());
        }
        Ok(buffer)
    }

    /// Reads up to `buffer.len()` decoded float samples into `buffer`,
    /// returning how many were written. Requires the channel to have been
    /// created with [`crate::flags::StreamFlags::FLOAT`] /
    /// [`crate::flags::MusicFlags::FLOAT`].
    fn get_data_f32(&self, buffer: &mut [f32]) -> Result<usize, BassError> {
        let byte_len = std::mem::size_of_val(buffer) as Dword;
        let flagged_len = byte_len | crate::ffi::consts::BASS_DATA_FLOAT;
        // SAFETY: `buffer` is writable for `byte_len` bytes; `BASS_DATA_FLOAT`
        // tells BASS the buffer holds `f32` samples rather than raw bytes.
        let result = unsafe {
            (self.lib().raw.bass_channel_get_data)(
                self.handle(),
                buffer.as_mut_ptr().cast(),
                flagged_len,
            )
        };
        if result < 0 {
            return Err(self.lib().last_error());
        }
        Ok(result as usize / std::mem::size_of::<f32>())
    }

    /// Registers `callback` to run once when the channel reaches the end.
    ///
    /// The callback runs on a BASS-internal thread; keep it fast and avoid
    /// blocking. Drop the returned [`ChannelSync`] to unregister early.
    fn on_end(&self, callback: impl Fn() + Send + 'static) -> Result<ChannelSync, BassError> {
        ChannelSync::register(Arc::clone(self.lib()), self.handle(), callback)
    }
}

/// A coarse, best-effort description of a `BASS_CHANNELINFO.ctype` value.
///
/// Only the top-level category bits are interpreted; sub-format codes
/// (exact codec) aren't decoded here since not every one is confidently
/// verifiable without a real `bass.h` (see the crate-level docs).
fn format_name(ctype: Dword) -> String {
    const CTYPE_STREAM_BASE: Dword = 0x10000;
    const CTYPE_MUSIC_BASE: Dword = 0x20000;
    const CATEGORY_MASK: Dword = 0xFFFF_0000;
    match ctype & CATEGORY_MASK {
        CTYPE_STREAM_BASE => "Stream".to_string(),
        CTYPE_MUSIC_BASE => "Music".to_string(),
        _ => format!("Unknown (0x{ctype:08x})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_name_recognises_stream_and_music_categories() {
        assert_eq!(format_name(0x10005), "Stream");
        assert_eq!(format_name(0x20001), "Music");
        assert_eq!(format_name(0x99999999), "Unknown (0x99999999)");
    }
}
