//! Spectrum renderer for the visualizer strip (#25): `bar_count`
//! logarithmically-spaced bars averaged from the channel's FFT magnitudes,
//! plus peak-hold caps that decay at a fixed rate.

use eframe::egui;

use crate::theme;

use super::PEAK_DECAY_PER_FRAME;

/// Number of bars across the strip. Within the issue's 24–48 range; the
/// strip is narrow, so 40 keeps each bar ~2 px wide.
const BAR_COUNT: usize = 40;
/// Fraction of each bar's slot filled by the bar (the rest is a gap).
const BAR_FILL: f32 = 0.72;
/// Peak cap thickness in points.
const CAP_HEIGHT: f32 = 1.5;
/// Perceptual scaling exponent: < 1 lifts quiet bins so most of the range
/// doesn't sit flat at the bottom.
const SCALE_EXPONENT: f32 = 0.5;

/// Draws `bins` as log-spaced bars. `peaks` is the peak-hold state, resized
/// and decayed in place.
pub fn draw(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    bins: &[f32],
    peaks: &mut Vec<f32>,
) {
    resize_peaks(peaks, BAR_COUNT);
    for peak in peaks.iter_mut() {
        *peak = (*peak - PEAK_DECAY_PER_FRAME).max(0.0);
    }
    if bins.is_empty() {
        return;
    }

    let values = bars_from_fft(bins, BAR_COUNT);
    let slot_width = rect.width() / BAR_COUNT as f32;
    let bar_width = (slot_width * BAR_FILL).max(1.0);
    let accent = theme::current_accent();
    let cap_color = ui.visuals().text_color();

    for (i, value) in values.iter().enumerate() {
        let x = rect.left() + i as f32 * slot_width;
        draw_bar(painter, rect, x, bar_width, *value, accent);
        peaks[i] = peaks[i].max(*value);
        draw_cap(painter, rect, x, bar_width, peaks[i], cap_color);
    }
}

/// A single accent-filled bar rising from `rect`'s bottom edge.
fn draw_bar(
    painter: &egui::Painter,
    rect: egui::Rect,
    x: f32,
    width: f32,
    value: f32,
    color: egui::Color32,
) {
    let height = rect.height() * value.clamp(0.0, 1.0);
    if height <= 0.0 {
        return;
    }
    let bar = egui::Rect::from_min_max(
        egui::pos2(x, rect.bottom() - height),
        egui::pos2(x + width, rect.bottom()),
    );
    painter.rect_filled(bar, 0.0, color);
}

/// The peak-hold cap: a thin line floating at the peak's height.
fn draw_cap(
    painter: &egui::Painter,
    rect: egui::Rect,
    x: f32,
    width: f32,
    peak: f32,
    color: egui::Color32,
) {
    if peak <= 0.0 {
        return;
    }
    let y = (rect.bottom() - rect.height() * peak.clamp(0.0, 1.0)).max(rect.top() + CAP_HEIGHT);
    let cap = egui::Rect::from_min_max(egui::pos2(x, y - CAP_HEIGHT), egui::pos2(x + width, y));
    painter.rect_filled(cap, 0.0, color);
}

/// Grows `peaks` to exactly `len`, keeping existing values. Called every
/// frame so a config change (or the first frame) is handled cheaply.
fn resize_peaks(peaks: &mut Vec<f32>, len: usize) {
    peaks.resize(len, 0.0);
}

/// Averages `bins` into `bar_count` perceptually-scaled bar values
/// (0.0..=1.0), using logarithmically-spaced frequency bands so low
/// frequencies (where music's energy lives) get as much width as high ones.
fn bars_from_fft(bins: &[f32], bar_count: usize) -> Vec<f32> {
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
                mean.max(0.0).powf(SCALE_EXPONENT).min(1.0)
            }
        })
        .collect()
}

/// The `[start, end)` FFT bin range for log-spaced `bar` of `bar_count`,
/// over `bin_count` bins. Bands use bins `1..bin_count` (bin 0 is DC).
fn band_range(bar: usize, bar_count: usize, bin_count: usize) -> (usize, usize) {
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

#[cfg(test)]
mod tests;
