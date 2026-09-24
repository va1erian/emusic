//! Milkdrop-mode renderer for the visualizer strip: paints the
//! [`emusic_milkdrop::Frame`] produced by the placeholder engine as a small
//! swirling, pulsing gradient of concentric rings.
//!
//! This stands in for a real MilkDrop/projectM engine (see
//! `emusic-milkdrop`'s crate docs) while keeping the same painter-only,
//! no-texture drawing style as [`super::spectrum`] and [`super::scope`].

use eframe::egui;
use emusic_milkdrop::Frame;

/// Number of concentric rings drawn per frame.
const RING_COUNT: usize = 5;
/// Saturation/value used for every ring's colour; only hue varies.
const SATURATION: f32 = 0.75;
const VALUE: f32 = 0.95;

/// Draws `frame` as `RING_COUNT` concentric, alpha-faded rings centred on
/// `rect`, sized by `frame.pulse` and rotated by `frame.swirl`.
pub fn draw(painter: &egui::Painter, rect: egui::Rect, frame: Frame) {
    let center = rect.center();
    let max_radius = rect.height() * 0.5;

    for i in 0..RING_COUNT {
        let t = i as f32 / RING_COUNT as f32;
        let hue = (frame.hue + t * 0.15).fract();
        let color: egui::Color32 = egui::ecolor::Hsva::new(hue, SATURATION, VALUE, 1.0).into();

        let radius = max_radius * (0.25 + 0.75 * frame.pulse) * (1.0 - t * 0.7);
        if radius <= 0.5 {
            continue;
        }
        let offset = egui::vec2((frame.swirl + t * 2.0).cos(), (frame.swirl + t * 2.0).sin())
            * (max_radius - radius).max(0.0)
            * 0.3;

        painter.circle_stroke(center + offset, radius, egui::Stroke::new(1.5, color));
    }
}
