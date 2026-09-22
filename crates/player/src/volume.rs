//! Perceptual volume curve.
//!
//! Human loudness perception is roughly logarithmic, so a linear UI slider
//! (`0.0..=1.0`) mapped straight onto BASS's linear `ATTRIB_VOL` gain feels
//! front-loaded: most of the audible change happens in the bottom
//! quarter of the slider. Squaring the input gives a cheap, dependency-free
//! approximation of a logarithmic (audio-taper) pot that spreads perceived
//! loudness more evenly across the slider's range.

/// Maps a linear UI volume (`0.0` silent .. `1.0` full) to the gain passed
/// to `BASS_ChannelSetAttribute(..., BASS_ATTRIB_VOL, ...)`.
pub fn perceptual_to_gain(volume: f32) -> f32 {
    volume.clamp(0.0, 1.0).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_preserved() {
        assert_eq!(perceptual_to_gain(0.0), 0.0);
        assert_eq!(perceptual_to_gain(1.0), 1.0);
    }

    #[test]
    fn curve_is_below_linear_in_the_middle() {
        // The whole point of the curve: a mid-slider position should be
        // quieter than a naive linear mapping would give.
        assert!(perceptual_to_gain(0.5) < 0.5);
    }

    #[test]
    fn out_of_range_input_is_clamped() {
        assert_eq!(perceptual_to_gain(-1.0), 0.0);
        assert_eq!(perceptual_to_gain(2.0), 1.0);
    }
}
