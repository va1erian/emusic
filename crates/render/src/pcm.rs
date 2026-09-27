#![forbid(unsafe_code)]

//! Interleaved signed 16-bit PCM, the common output of every renderer.

use std::time::Duration;

use crate::error::RenderError;

/// Interleaved signed 16-bit PCM at a given sample rate.
///
/// Samples are stored frame-major: `[left[0], right[0], left[1], right[1], …]`
/// for stereo. Mono renderers (SID) use one channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcmBuffer {
    sample_rate: u32,
    channels: u16,
    samples: Vec<i16>,
}

impl PcmBuffer {
    /// Builds a buffer, rejecting a zero sample rate/channel count or a sample
    /// count that is not a whole number of frames.
    pub fn new(sample_rate: u32, channels: u16, samples: Vec<i16>) -> Result<Self, RenderError> {
        if sample_rate == 0 {
            return Err(RenderError::InvalidPcm("sample rate is zero".into()));
        }
        if channels == 0 {
            return Err(RenderError::InvalidPcm("channel count is zero".into()));
        }
        if !samples.len().is_multiple_of(usize::from(channels)) {
            return Err(RenderError::InvalidPcm(format!(
                "{} samples is not a whole number of {channels}-channel frames",
                samples.len()
            )));
        }
        Ok(Self {
            sample_rate,
            channels,
            samples,
        })
    }

    /// Builds a mono buffer (the shape SID and most chip renderers produce).
    pub fn mono(sample_rate: u32, samples: Vec<i16>) -> Result<Self, RenderError> {
        Self::new(sample_rate, 1, samples)
    }

    /// The rendering sample rate, in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The number of interleaved channels.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// The interleaved samples.
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }

    /// The number of frames (per-channel samples).
    ///
    /// Always a whole number because [`PcmBuffer::new`] validated the shape.
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }

    /// Whether the buffer holds no samples.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// The playback duration.
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames() as f64 / f64::from(self.sample_rate))
    }

    /// The largest absolute sample value, used to tell silence from audio.
    pub fn peak(&self) -> u16 {
        self.samples
            .iter()
            .map(|sample| sample.unsigned_abs())
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_inconsistent_shapes() {
        assert!(PcmBuffer::new(0, 1, vec![0]).is_err());
        assert!(PcmBuffer::new(44_100, 0, vec![0]).is_err());
        assert!(PcmBuffer::new(44_100, 2, vec![0]).is_err());
    }

    #[test]
    fn reports_frames_duration_and_peak() {
        let buffer = PcmBuffer::new(44_100, 2, vec![0, -32768, 100, -100]).unwrap();
        assert_eq!(buffer.frames(), 2);
        assert_eq!(buffer.channels(), 2);
        assert_eq!(buffer.peak(), 32_768);
        assert_eq!(buffer.duration(), Duration::from_secs_f64(2.0 / 44_100.0));
    }
}
