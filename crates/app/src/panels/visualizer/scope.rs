//! Oscilloscope renderer for the visualizer strip (#25): the channel's raw
//! float samples as a single anti-aliased polyline.

use eframe::egui;

use emusic_ui::panels::visualizer::analysis::{MAX_POINTS, decimate};

use crate::theme;

/// Vertical gain applied to the samples before clamping to the strip, so a
/// quiet mix is still visible without amplifying a loud one past the edges.
const SCOPE_GAIN: f32 = 1.0;

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

#[cfg(test)]
mod tests;
