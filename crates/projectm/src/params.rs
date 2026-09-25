#![forbid(unsafe_code)]

//! [`Parameters`]: projectM's plain numeric settings, applied together.

/// Preset timing and audio-reaction settings, applied with
/// [`crate::Instance::set_parameters`]. Mirrors the frontend's persisted
/// visualization settings without depending on them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parameters {
    /// Seconds each preset shows before the playlist moves on.
    pub preset_duration_secs: f32,
    /// Seconds a soft (blended) transition takes.
    pub soft_cut_secs: f32,
    /// Whether loud beats may cut to the next preset immediately.
    pub hard_cuts: bool,
    /// How loud a beat must be to hard-cut.
    pub hard_cut_sensitivity: f32,
    /// How strongly presets react to beats.
    pub beat_sensitivity: f32,
    /// Random playlist order.
    pub shuffle: bool,
    /// Stay on the current preset.
    pub preset_locked: bool,
    /// The frame rate presets animate for.
    pub fps: u32,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            preset_duration_secs: 30.0,
            soft_cut_secs: 3.0,
            hard_cuts: false,
            hard_cut_sensitivity: 2.0,
            beat_sensitivity: 1.0,
            shuffle: true,
            preset_locked: false,
            fps: 60,
        }
    }
}
