//! Unit tests for the log-spaced FFT bar mapping.

use super::*;

#[test]
fn bars_from_fft_produces_the_requested_bar_count() {
    let bins = vec![0.5_f32; 512];
    let bars = bars_from_fft(&bins, 40);
    assert_eq!(bars.len(), 40);
}

#[test]
fn every_bar_gets_a_non_empty_band() {
    // With more bars than bins in the low end, bands must still cover at
    // least one bin each rather than collapsing to empty slices.
    let bins = vec![0.0_f32; 64];
    for bar in 0..40 {
        let (start, end) = band_range(bar, 40, bins.len());
        assert!(
            start < end,
            "bar {bar} mapped to empty range {start}..{end}"
        );
        assert!(end <= bins.len());
    }
}

#[test]
fn bands_are_monotonically_increasing_and_cover_the_spectrum() {
    let mut previous_end: usize = 1;
    for bar in 0..40 {
        let (start, end) = band_range(bar, 40, 512);
        assert!(start >= previous_end.saturating_sub(1));
        previous_end = end;
    }
    // The top band reaches the Nyquist bin.
    assert_eq!(band_range(39, 40, 512).1, 512);
}

#[test]
fn logarithmic_bands_are_wider_at_high_frequencies() {
    let low_width = {
        let (s, e) = band_range(4, 40, 512);
        e - s
    };
    let high_width = {
        let (s, e) = band_range(39, 40, 512);
        e - s
    };
    assert!(
        high_width > low_width,
        "high band {high_width} should exceed low band {low_width}"
    );
}

#[test]
fn perceptual_scaling_lifts_quiet_bins() {
    let mut bins = vec![0.01_f32; 512];
    bins[0] = 1.0;
    let bars = bars_from_fft(&bins, 1);
    // sqrt(0.01) = 0.1, far above the raw 0.01.
    assert!(bars[0] > 0.05, "got {}", bars[0]);
}

#[test]
fn values_are_clamped_to_unit_range() {
    let bars = bars_from_fft(&vec![100.0_f32; 512], 40);
    for bar in bars {
        assert!((0.0..=1.0).contains(&bar), "bar {bar} out of range");
    }
}

#[test]
fn empty_input_is_safe() {
    assert!(bars_from_fft(&[], 40).is_empty());
}

#[test]
fn resize_peaks_grows_and_keeps_existing() {
    let mut peaks = vec![0.7, 0.3];
    resize_peaks(&mut peaks, 4);
    assert_eq!(peaks.len(), 4);
    assert_eq!(peaks[0], 0.7);
    assert_eq!(peaks[3], 0.0);
}

#[test]
fn peak_decay_is_linear_and_never_negative() {
    let dt = 1.0 / 60.0;
    let decay = PEAK_DECAY_PER_SECOND * dt;
    let mut peak = 1.0_f32;
    for _ in 0..3 {
        peak = (peak - decay).max(0.0);
    }
    assert!((peak - (1.0 - 3.0 * decay)).abs() < 1e-6);
    for _ in 0..200 {
        peak = (peak - decay).max(0.0);
    }
    assert_eq!(peak, 0.0);
}
