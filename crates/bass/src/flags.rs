//! Safe flag/enum types mirroring the raw BASS bit constants in
//! [`crate::ffi::consts`].

use bitflags::bitflags;

use crate::ffi::consts as c;
use crate::ffi::types::Dword;

bitflags! {
    /// Flags for [`crate::Stream::from_file`] (`BASS_StreamCreateFile`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct StreamFlags: u32 {
        /// Decode to 32-bit floating point instead of 16-bit integer.
        const FLOAT = c::BASS_SAMPLE_FLOAT;
        /// Downmix to mono.
        const MONO = c::BASS_SAMPLE_MONO;
        /// Loop the stream when it reaches the end.
        const LOOP = c::BASS_SAMPLE_LOOP;
        /// Pre-scan the file to get an accurate length/seek table.
        const PRESCAN = c::BASS_STREAM_PRESCAN;
        /// Free the stream automatically once playback reaches the end.
        const AUTOFREE = c::BASS_STREAM_AUTOFREE;
        /// Create a decoding stream (no playback device output).
        const DECODE = c::BASS_STREAM_DECODE;
    }
}

bitflags! {
    /// Flags for [`crate::Bass::open_push_stream`] (`BASS_StreamCreate`
    /// with `STREAMPROC_PUSH`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct PushFlags: u32 {
        /// Feed 8-bit unsigned samples instead of the default signed 16-bit.
        const EIGHT_BIT = c::BASS_SAMPLE_8BITS;
        /// Feed 32-bit floating-point samples instead of 16-bit integer.
        const FLOAT = c::BASS_SAMPLE_FLOAT;
        /// Downmix to mono.
        const MONO = c::BASS_SAMPLE_MONO;
        /// Loop the stream when it reaches the end.
        const LOOP = c::BASS_SAMPLE_LOOP;
        /// Create a decoding stream (no playback device output).
        const DECODE = c::BASS_STREAM_DECODE;
        /// Free the stream automatically once playback reaches the end.
        const AUTOFREE = c::BASS_STREAM_AUTOFREE;
    }
}

bitflags! {
    /// Flags for [`crate::Music::from_file`] (`BASS_MusicLoad`) and for
    /// [`crate::Channel::set_flags`] on a music channel
    /// (`BASS_ChannelFlags`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct MusicFlags: u32 {
        /// Loop the music when it reaches the end.
        const LOOP = c::BASS_MUSIC_LOOP;
        /// No interpolation for the samples' mixing (harsh, "chip" sound).
        const NONINTER = c::BASS_MUSIC_NONINTER;
        /// Sinc interpolation, higher quality than the default linear.
        const SINCINTER = c::BASS_MUSIC_SINCINTER;
        /// Enable ramping to smooth volume/panning changes.
        const RAMP = c::BASS_MUSIC_RAMP;
        /// "Sensitive" ramping, closer to a tracker's own behaviour.
        const RAMPS = c::BASS_MUSIC_RAMPS;
        /// Surround sound mode 1.
        const SURROUND = c::BASS_MUSIC_SURROUND;
        /// Surround sound mode 2.
        const SURROUND2 = c::BASS_MUSIC_SURROUND2;
        /// Apply FastTracker 2's 100% panning to MOD files (alias of
        /// [`Self::FT2PAN`] — BASS defines them as the same bit).
        const FT2MOD = c::BASS_MUSIC_FT2MOD;
        /// See [`Self::FT2MOD`].
        const FT2PAN = c::BASS_MUSIC_FT2PAN;
        /// Play .MOD files as ProTracker 1 would.
        const PT1MOD = c::BASS_MUSIC_PT1MOD;
        /// Stop all notes when moving position (avoids hanging notes).
        const POSRESET = c::BASS_MUSIC_POSRESET;
        /// Stop the music when a backward jump effect is hit (otherwise it
        /// would loop indefinitely).
        const STOPBACK = c::BASS_MUSIC_STOPBACK;
        /// Pre-scan for an accurate length (`BASS_ChannelGetLength`).
        const PRESCAN = c::BASS_MUSIC_PRESCAN;
        /// Don't load the samples. Reduces memory use; useful when only
        /// metadata (name, message, instrument/sample names) is needed.
        const NOSAMPLE = c::BASS_MUSIC_NOSAMPLE;
        /// Decode to 32-bit floating point.
        const FLOAT = c::BASS_SAMPLE_FLOAT;
        /// Create a decoding channel (no device output).
        const DECODE = c::BASS_STREAM_DECODE;
    }
}

/// A channel attribute settable/gettable via `BASS_ChannelGetAttribute` /
/// `BASS_ChannelSetAttribute`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    /// Playback sample rate, in Hz.
    Freq,
    /// Volume level, `0.0` (silent) to `1.0` (full).
    Volume,
    /// Panning position, `-1.0` (full left) to `1.0` (full right).
    Pan,
    /// Maximum number of bytes a push stream may have queued (`0` = no
    /// limit); push streams only.
    PushLimit,
    /// Music amplification level, `0`-`100` (MOD music channels only).
    MusicAmplify,
    /// Music channel separation, `0`-`100` (MOD music channels only; `0`
    /// uses BASS's default).
    MusicPanSeparation,
}

impl Attribute {
    pub(crate) fn as_raw(self) -> Dword {
        match self {
            Self::Freq => c::BASS_ATTRIB_FREQ,
            Self::Volume => c::BASS_ATTRIB_VOL,
            Self::Pan => c::BASS_ATTRIB_PAN,
            Self::PushLimit => c::BASS_ATTRIB_PUSH_LIMIT,
            Self::MusicAmplify => c::BASS_ATTRIB_MUSIC_AMPLIFY,
            Self::MusicPanSeparation => c::BASS_ATTRIB_MUSIC_PANSEP,
        }
    }
}

/// Which position/length units to use for a channel query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionMode {
    /// Position/length in bytes of decoded PCM data.
    Bytes,
    /// Position in "orders" (MOD music channels only).
    MusicOrder,
}

impl PositionMode {
    pub(crate) fn as_raw(self) -> Dword {
        match self {
            Self::Bytes => c::BASS_POS_BYTE,
            Self::MusicOrder => c::BASS_POS_MUSIC_ORDER,
        }
    }
}

/// A channel's playback state, from `BASS_ChannelIsActive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Not playing.
    Stopped,
    /// Playing.
    Playing,
    /// Playing but stalled waiting for more data (e.g. network buffering).
    Stalled,
    /// Paused.
    Paused,
    /// Paused because the output device is paused/unavailable.
    PausedDevice,
}

impl PlaybackState {
    pub(crate) fn from_raw(value: Dword) -> Self {
        match value {
            c::BASS_ACTIVE_PLAYING => Self::Playing,
            c::BASS_ACTIVE_STALLED => Self::Stalled,
            c::BASS_ACTIVE_PAUSED => Self::Paused,
            c::BASS_ACTIVE_PAUSED_DEVICE => Self::PausedDevice,
            _ => Self::Stopped,
        }
    }
}

/// Number of points fed into the FFT for [`crate::Channel::get_data_fft`].
///
/// The number of magnitude values returned is half the point count (BASS
/// only returns positive frequencies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FftSize {
    Fft256,
    Fft512,
    Fft1024,
    Fft2048,
    Fft4096,
    Fft8192,
    Fft16384,
    Fft32768,
}

impl FftSize {
    pub(crate) fn as_raw(self) -> Dword {
        match self {
            Self::Fft256 => c::BASS_DATA_FFT256,
            Self::Fft512 => c::BASS_DATA_FFT512,
            Self::Fft1024 => c::BASS_DATA_FFT1024,
            Self::Fft2048 => c::BASS_DATA_FFT2048,
            Self::Fft4096 => c::BASS_DATA_FFT4096,
            Self::Fft8192 => c::BASS_DATA_FFT8192,
            Self::Fft16384 => c::BASS_DATA_FFT16384,
            Self::Fft32768 => c::BASS_DATA_FFT32768,
        }
    }

    /// Number of `f32` magnitude values BASS writes for this size.
    pub fn output_len(self) -> usize {
        let points = match self {
            Self::Fft256 => 256,
            Self::Fft512 => 512,
            Self::Fft1024 => 1024,
            Self::Fft2048 => 2048,
            Self::Fft4096 => 4096,
            Self::Fft8192 => 8192,
            Self::Fft16384 => 16384,
            Self::Fft32768 => 32768,
        };
        points / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_flags_combine_with_bitor() {
        let flags = MusicFlags::RAMP | MusicFlags::LOOP;
        assert!(flags.contains(MusicFlags::RAMP));
        assert!(flags.contains(MusicFlags::LOOP));
        assert!(!flags.contains(MusicFlags::SURROUND));
    }

    #[test]
    fn ft2mod_and_ft2pan_are_the_same_bit() {
        assert_eq!(MusicFlags::FT2MOD.bits(), MusicFlags::FT2PAN.bits());
    }

    #[test]
    fn fft_output_len_is_half_the_point_count() {
        assert_eq!(FftSize::Fft1024.output_len(), 512);
        assert_eq!(FftSize::Fft256.output_len(), 128);
    }

    #[test]
    fn playback_state_defaults_to_stopped_for_unknown_values() {
        assert_eq!(PlaybackState::from_raw(999), PlaybackState::Stopped);
        assert_eq!(PlaybackState::from_raw(1), PlaybackState::Playing);
    }

    #[test]
    fn push_flags_combine_with_bitor() {
        let flags = PushFlags::FLOAT | PushFlags::DECODE;
        assert!(flags.contains(PushFlags::FLOAT));
        assert!(flags.contains(PushFlags::DECODE));
        assert!(!flags.contains(PushFlags::LOOP));
    }

    #[test]
    fn push_limit_attribute_matches_the_header() {
        assert_eq!(Attribute::PushLimit.as_raw(), 17);
    }
}
