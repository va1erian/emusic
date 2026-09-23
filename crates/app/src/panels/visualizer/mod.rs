//! Lightweight visualizer strip in the status bar (#25).
//!
//! Two modes are drawn with a single [`egui::Painter`] pass of rects/lines —
//! no textures: [`VisualizerMode::Spectrum`] (log-spaced bars from the
//! channel's FFT with peak-hold caps) and [`VisualizerMode::Oscilloscope`]
//! (the channel's raw float samples as one trace). Clicking the strip cycles
//! spectrum → oscilloscope → off; the choice is persisted via
//! [`crate::config::Config`].
//!
//! Split by responsibility:
//! - `mod.rs` (this file) — the strip widget, its state (peak caps) and the
//!   click-to-cycle behaviour.
//! - [`spectrum`] — the log-spaced FFT bar renderer.
//! - [`scope`] — the oscilloscope trace renderer.

mod scope;
mod spectrum;

use std::time::Duration;

use eframe::egui;

use crate::player_api::{PlaybackStatus, PlayerApi};
use crate::state::{AppState, Command, VisualizerMode};

/// Strip size in the status bar, in points.
const STRIP_SIZE: egui::Vec2 = egui::vec2(150.0, 18.0);

/// Target animation rate for the strip while a mode is active, decoupled
/// from the compositor's own refresh rate (#25 follow-up). A full frame
/// re-lays out and repaints the whole window, so chasing vsync on a
/// 120/144 Hz display multiplied that cost for no visible benefit — the
/// strip is 18 px tall and doesn't need to be smoother than this to read as
/// reactive. [`App::update`](crate::app::App::update) uses this to bound
/// `request_repaint_after` instead of calling `request_repaint()`.
pub const FRAME_INTERVAL: Duration = Duration::from_millis(33);

/// Decay rate of the spectrum's peak-hold caps, in units/second (a
/// full-height cap drains in a little over a second). Applied scaled by the
/// actual frame delta rather than a fixed per-frame amount, so the caps
/// fall at the same visual speed regardless of [`FRAME_INTERVAL`] or any
/// frame that arrives late.
const PEAK_DECAY_PER_SECOND: f32 = 0.9;

/// Transient visualizer state that must survive across frames: the
/// spectrum's peak-hold caps. UI-only, so it is deliberately not persisted.
#[derive(Debug, Default)]
pub struct VisualizerState {
    /// Peak-hold value per bar, 0.0..=1.0.
    peaks: Vec<f32>,
}

impl VisualizerState {
    /// Whether the strip needs fresh audio data this frame. Only true while
    /// a mode is active *and* audio is actually playing — so a paused/stopped
    /// player (or `Off`) never touches the audio engine.
    pub fn wishes_repaint(mode: VisualizerMode, playing: bool) -> bool {
        playing && mode != VisualizerMode::Off
    }
}

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
