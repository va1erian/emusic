//! Push streams (`BASS_StreamCreate` with `STREAMPROC_PUSH`): BASS plays or
//! decodes PCM that the owner feeds in, instead of pulling it from a file or
//! a user callback.
//!
//! This is the building block for playing formats BASS can't decode itself
//! (a SID player, a software synth, ...): decode on your own thread and hand
//! the samples to [`PushStream::push_data`].
//!
//! # Threading contract
//!
//! BASS is documented as thread-safe, and the feeding calls here are meant
//! to be called from a decoder/feeder thread while the UI thread drives
//! playback:
//!
//! - [`PushStream::push_data`], [`PushStream::end_of_stream`] and
//!   [`PushStream::queued_bytes`] may be called from any thread, including
//!   concurrently with each other and with the [`Channel`] playback methods.
//! - A feeder should keep an eye on [`PushStream::queued_bytes`] (against
//!   [`PushStream::push_limit`], if set) and top the queue up as it drains;
//!   [`Channel::is_active`] reporting [`PlaybackState::Stalled`] means BASS
//!   has run dry.
//! - [`PushStream::set_duration`] is only intended to be called by the
//!   thread that owns the stream, but is internally synchronised so reads
//!   from other threads are safe.

use std::ffi::c_void;
use std::ptr;
use std::sync::{Arc, Mutex, PoisonError};

use crate::channel::Channel;
use crate::error::BassError;
use crate::ffi::types::{Dword, HStream};
use crate::ffi::{BassLib, consts as c};
use crate::flags::{Attribute, PositionMode, PushFlags};

/// A playable or decoding "push" stream, created with `BASS_StreamCreate`
/// and the `STREAMPROC_PUSH` sentinel.
///
/// Freed automatically (`BASS_StreamFree`) when dropped.
///
/// See the [module docs](self) for the threading contract.
pub struct PushStream {
    lib: Arc<BassLib>,
    handle: HStream,
    /// Sample rate, in Hz.
    freq: u32,
    /// Bytes in one sample frame (`channels * bytes_per_sample`).
    frame_bytes: u32,
    /// Owner-supplied known duration, in seconds.
    duration: Mutex<Option<f64>>,
}

impl PushStream {
    /// Creates a push stream. `freq` is the sample rate in Hz and `channels`
    /// the channel count (`1` = mono, `2` = stereo, ...).
    pub(crate) fn create(
        lib: Arc<BassLib>,
        freq: u32,
        channels: u32,
        flags: PushFlags,
    ) -> Result<Self, BassError> {
        // SAFETY: `STREAMPROC_PUSH` is the documented `(STREAMPROC *)-1`
        // sentinel; the `user` argument is unused for push streams, so a
        // null pointer is fine. `freq`/`channels`/`flags` are plain values
        // BASS validates itself.
        let handle = unsafe {
            (lib.raw.bass_stream_create)(
                freq,
                channels,
                flags.bits(),
                c::STREAMPROC_PUSH,
                ptr::null_mut(),
            )
        };
        if handle == 0 {
            return Err(lib.last_error());
        }
        Ok(Self {
            lib,
            handle,
            freq,
            frame_bytes: bytes_per_frame(channels, flags),
            duration: Mutex::new(None),
        })
    }

    /// Pushes decoded PCM bytes into the stream's queue.
    ///
    /// `data` must contain a whole number of sample frames for the stream's
    /// format (see [`PushStream::frame_bytes`]); otherwise
    /// [`BassError::IllParam`] is returned without calling BASS.
    ///
    /// Returns the number of bytes currently queued, which may be less than
    /// `data.len()` if a [`PushStream::push_limit`] is in effect. The call
    /// can be made from a decoder thread; see the [module docs](self).
    pub fn push_data(&self, data: &[u8]) -> Result<u32, BassError> {
        if !is_whole_frames(data.len(), self.frame_bytes) {
            return Err(BassError::IllParam);
        }
        self.put(data.as_ptr().cast(), data.len() as Dword)
    }

    /// Signals that no more data will be pushed: once the queued data has
    /// played, the stream ends.
    ///
    /// Safe to call from a decoder thread; see the [module docs](self).
    pub fn end_of_stream(&self) -> Result<(), BassError> {
        // A dangling-but-non-null pointer with a zero data length, plus the
        // `BASS_STREAMPROC_END` flag, mirrors a `STREAMPROC` returning 0
        // bytes and the end flag. BASS copies no bytes from the pointer.
        self.put((&[] as &[u8]).as_ptr().cast(), c::BASS_STREAMPROC_END)?;
        Ok(())
    }

    /// The number of bytes currently queued in the stream's push buffer.
    ///
    /// A feeder thread can poll this to decide when to push more. Safe to
    /// call from any thread; see the [module docs](self).
    pub fn queued_bytes(&self) -> Result<u32, BassError> {
        // A null buffer with a zero length asks BASS to report the queue
        // level without allocating or consuming anything.
        self.put(ptr::null(), 0)
    }

    /// The size of one sample frame, in bytes, for this stream's format.
    pub fn frame_bytes(&self) -> u32 {
        self.frame_bytes
    }

    /// Sets the maximum number of bytes the stream may have queued; `0`
    /// means no limit.
    ///
    /// Useful to stop a feeder from growing the queue without bound when
    /// playback is paused. Push streams only.
    pub fn set_push_limit(&self, bytes: u32) -> Result<(), BassError> {
        self.set_attribute(Attribute::PushLimit, bytes as f32)
    }

    /// The current push queue limit in bytes (`0` = no limit).
    pub fn push_limit(&self) -> Result<u32, BassError> {
        // The attribute is an integer; BASS stores it as a float.
        Ok(self.attribute(Attribute::PushLimit)? as u32)
    }

    /// Supplies a known total duration, in seconds, for the stream.
    ///
    /// `BASS_ChannelGetLength` can't report a length for a push stream, so
    /// the owner tells us how long the audio will be and [`Channel::length`]
    /// / [`Channel::length_seconds`] report that instead. Negative values
    /// are clamped to zero.
    pub fn set_duration(&self, seconds: f64) {
        *self.duration.lock().unwrap_or_else(PoisonError::into_inner) = Some(seconds.max(0.0));
    }

    /// The owner-supplied duration, in seconds, if one has been set.
    pub fn duration(&self) -> Option<f64> {
        *self.duration.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn put(&self, buffer: *const c_void, length: Dword) -> Result<u32, BassError> {
        // SAFETY: `self.handle` names a push stream owned by `self`. For a
        // non-null `buffer`, the caller guarantees it is valid for `length`
        // bytes (masked to exclude the end flag); `queued_bytes` and
        // `end_of_stream` pass null/empty pointers with lengths that make
        // BASS read no data.
        let queued = unsafe { (self.lib.raw.bass_stream_put_data)(self.handle, buffer, length) };
        if queued == Dword::MAX {
            Err(self.lib.last_error())
        } else {
            Ok(queued)
        }
    }
}

impl Channel for PushStream {
    fn handle(&self) -> Dword {
        self.handle
    }

    fn lib(&self) -> &Arc<BassLib> {
        &self.lib
    }

    fn length(&self, mode: PositionMode) -> Result<u64, BassError> {
        match mode {
            PositionMode::Bytes => {
                let seconds = self.duration().ok_or(BassError::NotAvail)?;
                Ok(seconds_to_bytes(seconds, self.freq, self.frame_bytes))
            }
            PositionMode::MusicOrder => Err(BassError::NotAvail),
        }
    }

    fn length_seconds(&self) -> Result<f64, BassError> {
        self.duration().ok_or(BassError::NotAvail)
    }
}

impl Drop for PushStream {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was returned by a successful
        // `BASS_StreamCreate` call and hasn't been freed yet (nothing else
        // can free it: `PushStream` owns it exclusively).
        unsafe { (self.lib.raw.bass_stream_free)(self.handle) };
    }
}

/// Bytes per sample for a push stream's format: 8-bit, 16-bit or `f32`.
fn bytes_per_frame(channels: u32, flags: PushFlags) -> u32 {
    let sample_bytes = if flags.contains(PushFlags::FLOAT) {
        4
    } else if flags.contains(PushFlags::EIGHT_BIT) {
        1
    } else {
        2
    };
    channels.max(1) * sample_bytes
}

/// Whether `len` bytes form a whole number of sample frames.
fn is_whole_frames(len: usize, frame_bytes: u32) -> bool {
    frame_bytes != 0 && len.is_multiple_of(frame_bytes as usize)
}

/// Converts a duration in seconds to a byte count for a given format.
fn seconds_to_bytes(seconds: f64, freq: u32, frame_bytes: u32) -> u64 {
    if seconds <= 0.0 || seconds.is_nan() {
        return 0;
    }
    (seconds * f64::from(freq) * f64::from(frame_bytes)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_per_frame_uses_the_sample_width() {
        assert_eq!(bytes_per_frame(1, PushFlags::FLOAT), 4);
        assert_eq!(bytes_per_frame(2, PushFlags::empty()), 4);
        assert_eq!(bytes_per_frame(2, PushFlags::EIGHT_BIT), 2);
        // Float wins if both width flags are (incorrectly) set.
        assert_eq!(
            bytes_per_frame(1, PushFlags::FLOAT | PushFlags::EIGHT_BIT),
            4
        );
        // A zero channel count is treated as mono rather than producing 0.
        assert_eq!(bytes_per_frame(0, PushFlags::FLOAT), 4);
    }

    #[test]
    fn whole_frames_requires_an_exact_multiple() {
        assert!(is_whole_frames(0, 4));
        assert!(is_whole_frames(8, 4));
        assert!(!is_whole_frames(6, 4));
        assert!(!is_whole_frames(4, 0));
    }

    #[test]
    fn seconds_to_bytes_scales_with_the_format() {
        assert_eq!(seconds_to_bytes(1.0, 44_100, 4), 176_400);
        assert_eq!(seconds_to_bytes(0.5, 44_100, 2), 44_100);
        assert_eq!(seconds_to_bytes(0.0, 44_100, 4), 0);
        assert_eq!(seconds_to_bytes(-1.0, 44_100, 4), 0);
    }

    #[test]
    fn streamproc_push_is_minus_one() {
        assert_eq!(c::STREAMPROC_PUSH as usize, usize::MAX);
    }

    #[test]
    fn end_flag_matches_the_header() {
        assert_eq!(c::BASS_STREAMPROC_END, 0x8000_0000);
    }
}
