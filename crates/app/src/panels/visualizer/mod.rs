//! Lightweight visualizer strip in the status bar (#25).
//!
//! Two modes are drawn with a single [`egui::Painter`] pass of rects/lines —
//! no textures: [`VisualizerMode::Spectrum`] (log-spaced bars from the
//! channel's FFT with peak-hold caps) and [`VisualizerMode::Oscilloscope`]
//! (the channel's raw float samples as one trace). Clicking the strip cycles
//! spectrum → oscilloscope → off; the choice is persisted via
//! [`crate::config::Config`]. MilkDrop visuals are the projectM
//! visualization's job (#295), not the strip's.
//!
//! Split by responsibility:
//! - `mod.rs` (this file) — the strip widget, its state (peak caps) and the
//!   click-to-cycle behaviour.
//! - [`spectrum`] — the log-spaced FFT bar renderer.
//! - [`scope`] — the oscilloscope trace renderer.

mod scope;
mod spectrum;

use eframe::egui;

use crate::player_api::{PlaybackStatus, PlayerApi};
use crate::state::{AppState, Command, VisualizerMode};

pub use emusic_ui::panels::visualizer::{FRAME_INTERVAL, VisualizerState};

/// Strip size in the status bar, in points.
const STRIP_SIZE: egui::Vec2 = egui::vec2(150.0, 18.0);

/// Draws the visualizer strip, cycling the mode when clicked.
///
/// `state` supplies the persisted mode and the peak-hold caps; `player` is
/// only queried for FFT/samples when the current mode needs them.
pub fn show(ui: &mut egui::Ui, state: &mut AppState, player: &dyn PlayerApi) {
    let mode = state.visualizer;
    let (rect, response) = ui.allocate_exact_size(STRIP_SIZE, egui::Sense::click());
    if response.clicked() {
        state.push(Command::CycleVisualizer);
    }
    response.on_hover_text(visualizer_tooltip(mode));

    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);

    let playing = player.status() == PlaybackStatus::Playing;
    if !VisualizerState::wishes_repaint(mode, playing) {
        return;
    }
    let dt = ui.input(|i| i.stable_dt);
    match mode {
        VisualizerMode::Spectrum => {
            let bins = player.fft();
            spectrum::draw(
                ui,
                painter,
                rect,
                &bins,
                &mut state.visualizer_state.peaks,
                dt,
            );
        }
        VisualizerMode::Oscilloscope => {
            let samples = player.samples();
            scope::draw(painter, rect, &samples);
        }
        VisualizerMode::Off => {}
    }
}

fn visualizer_tooltip(mode: VisualizerMode) -> String {
    let next = mode.next();
    format!("Visualizer: {} (click for {})", mode.label(), next.label())
}

#[cfg(test)]
mod tests;
