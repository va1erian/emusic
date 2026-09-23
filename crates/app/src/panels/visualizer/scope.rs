//! Oscilloscope renderer for the visualizer strip (#25): the channel's raw
//! float samples as a single anti-aliased polyline.

use eframe::egui;

use crate::theme;

/// Vertical gain applied to the samples before clamping to the strip, so a
/// quiet mix is still visible without amplifying a loud one past the edges.
const SCOPE_GAIN: f32 = 1.0;
/// Maximum number of sample points turned into segment vertices. More than
/// this and the line is decimated, which keeps the painter pass cheap and
/// the trace legible in an 18 px-tall strip.
const MAX_POINTS: usize = 256;

/// Draws `samples` as a trace centred on `rect`'s vertical midpoint.
pub fn draw(painter: &egui::Painter, rect: egui::Rect, samples: &[f32]) {
    if samples.len() < 2 {
        return;
    }
    let points = decimate(samples, MAX_POINTS);
    let mid = rect.center().y;
    let half = rect.height() * 0.5;
    let step = rect.width() / (points.len() - 1) as f32;

    let vertices: Vec<egui::Pos2> = points
        .iter()
        .enumerate()
        .map(|(i, &sample)| {
            let x = rect.left() + i as f32 * step;
            let y = mid - (sample * SCOPE_GAIN).clamp(-1.0, 1.0) * half;
            egui::pos2(x, y)
        })
        .collect();
    painter.add(egui::Shape::line(
        vertices,
        egui::Stroke::new(1.0, theme::current_accent()),
    ));
}

/// Reduces `samples` to at most `max_points` by averaging adjacent windows,
/// preserving the overall waveform envelope.
fn decimate(samples: &[f32], max_points: usize) -> Vec<f32> {
    if samples.len() <= max_points {
        return samples.to_vec();
    }
    let window = samples.len().div_ceil(max_points);
    samples
        .chunks(window)
        .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
        .collect()
}

#[cfg(test)]
mod tests;
