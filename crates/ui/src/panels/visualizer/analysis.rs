//! Toolkit-agnostic maths behind the visualizer strip (#25): the spectrum's
//! log-spaced bar bands and peak-hold caps, and the oscilloscope's
//! decimation. The app draws what these return.

/// Number of bars across the strip. Kept low (24, the coarse end of the
/// issue's 24–48 range) so the bars read as chunky blocks rather than a
/// fine-grained comb.
pub const BAR_COUNT: usize = 24;
/// Perceptual scaling exponent: < 1 lifts quiet bins so most of the range
/// doesn't sit flat at the bottom.
const SCALE_EXPONENT: f32 = 0.4;
/// Overall gain applied after the perceptual scaling, so bars react strongly
/// to quiet passages instead of hugging the bottom of the strip.
const GAIN: f32 = 1.8;
/// Decay rate of the spectrum's peak-hold caps, in units/second (a
/// full-height cap drains in a little over a second). Applied scaled by the
/// actual frame delta, so the caps fall at the same visual speed regardless
/// of the frame rate.
pub const PEAK_DECAY_PER_SECOND: f32 = 0.9;
/// Maximum number of sample points in an oscilloscope trace. More than this
/// and the line is decimated, which keeps the paint cheap and the trace
/// legible in a small strip.
pub const MAX_POINTS: usize = 256;

/// Grows `peaks` to exactly `len`, keeping existing values. Called every
/// frame so a config change (or the first frame) is handled cheaply.
pub fn resize_peaks(peaks: &mut Vec<f32>, len: usize) {
    peaks.resize(len, 0.0);
}

/// Averages `bins` into `bar_count` perceptually-scaled bar values
/// (0.0..=1.0), using logarithmically-spaced frequency bands so low
/// frequencies (where music's energy lives) get as much width as high ones.
pub fn bars_from_fft(bins: &[f32], bar_count: usize) -> Vec<f32> {
    if bins.is_empty() || bar_count == 0 {
        return Vec::new();
    }
    (0..bar_count)
        .map(|bar| {
            let (start, end) = band_range(bar, bar_count, bins.len());
            let slice = &bins[start..end];
            if slice.is_empty() {
                0.0
            } else {
                let mean = slice.iter().sum::<f32>() / slice.len() as f32;
                (mean.max(0.0).powf(SCALE_EXPONENT) * GAIN).min(1.0)
            }
        })
        .collect()
}

/// The `[start, end)` FFT bin range for log-spaced `bar` of `bar_count`,
/// over `bin_count` bins. Bands use bins `1..bin_count` (bin 0 is DC).
pub fn band_range(bar: usize, bar_count: usize, bin_count: usize) -> (usize, usize) {
    if bin_count < 2 || bar_count == 0 {
        return (0, bin_count);
    }
    // Exponentiate from just above DC (bin 1) to the Nyquist bin.
    let low = 1.0_f32;
    let high = bin_count as f32;
    let ratio = high / low;
    let start_f = low * ratio.powf(bar as f32 / bar_count as f32);
    let end_f = low * ratio.powf((bar + 1) as f32 / bar_count as f32);
    let start = (start_f.floor() as usize).clamp(1, bin_count - 1);
    let end = (end_f.ceil() as usize).clamp(start + 1, bin_count);
    (start, end)
}

/// Reduces `samples` to at most `max_points` by averaging adjacent windows,
/// preserving the overall waveform envelope.
pub fn decimate(samples: &[f32], max_points: usize) -> Vec<f32> {
    if samples.len() <= max_points {
        return samples.to_vec();
    }
    let window = samples.len().div_ceil(max_points);
    samples
        .chunks(window)
        .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
        .collect()
}

/// Lowers every peak-hold cap by `dt` seconds' worth of decay, never below 0.
pub fn decay_peaks(peaks: &mut [f32], dt: f32) {
    let decay = PEAK_DECAY_PER_SECOND * dt;
    for peak in peaks {
        *peak = (*peak - decay).max(0.0);
    }
}
