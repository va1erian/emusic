//! Visualizer strip state shared with the shell (#25, #97).
//!
//! Rendering the spectrum/oscilloscope/milkdrop stays in the frontends;
//! here live the per-frame target rate the shell's repaint policy needs and
//! the transient state (peak-hold caps, the milkdrop placeholder engine)
//! that must survive across frames.

pub mod analysis;
use std::time::Duration;

use emusic_milkdrop::PlaceholderEngine;

use crate::state::VisualizerMode;

/// Target animation rate for the strip while a mode is active, decoupled
/// from the compositor's own refresh rate (#25 follow-up). A full frame
/// re-lays out and repaints the whole window, so chasing vsync on a
/// 120/144 Hz display multiplied that cost for no visible benefit — the
/// strip is 18 px tall and doesn't need to be smoother than this to read as
/// reactive.
pub const FRAME_INTERVAL: Duration = Duration::from_millis(33);

/// Transient visualizer state that must survive across frames: the
/// spectrum's peak-hold caps and the milkdrop engine. UI-only, so it is
/// deliberately not persisted.
#[derive(Debug, Default)]
pub struct VisualizerState {
    /// Peak-hold value per bar, 0.0..=1.0. Public so the frontend's spectrum
    /// renderer can draw and decay them.
    pub peaks: Vec<f32>,
    /// The [`VisualizerMode::Milkdrop`] engine. Public so the frontend's
    /// renderer can feed it PCM and tick it each frame.
    pub milkdrop: PlaceholderEngine,
}

impl VisualizerState {
    /// Whether the strip needs fresh audio data this frame. Only true while
    /// a mode is active *and* audio is actually playing — so a paused/stopped
    /// player (or `Off`) never touches the audio engine.
    pub fn wishes_repaint(mode: VisualizerMode, playing: bool) -> bool {
        playing && mode != VisualizerMode::Off
    }
}
