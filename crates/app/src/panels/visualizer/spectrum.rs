//! Spectrum renderer for the visualizer strip (#25): `bar_count`
//! logarithmically-spaced bars averaged from the channel's FFT magnitudes,
//! plus peak-hold caps that decay at a fixed rate.

use eframe::egui;

use crate::theme;

use emusic_ui::panels::visualizer::analysis::{
    BAR_COUNT, bars_from_fft, decay_peaks, resize_peaks,
};

/// Fraction of each bar's slot filled by the bar (the rest is a gap).
const BAR_FILL: f32 = 0.72;
/// Peak cap thickness in points.
const CAP_HEIGHT: f32 = 1.5;

/// Draws `bins` as log-spaced bars. `peaks` is the peak-hold state, resized
/// and decayed in place; `dt` is the elapsed time since the last frame, in
/// seconds, so the decay rate holds regardless of the actual frame rate.
pub fn draw(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    bins: &[f32],
    peaks: &mut Vec<f32>,
    dt: f32,
) {
    resize_peaks(peaks, BAR_COUNT);
    decay_peaks(peaks, dt);
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

#[cfg(test)]
mod tests;
