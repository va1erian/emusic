//! projectM engine and preset settings (#300), edited in Settings →
//! Visualization and applied live to a running instance.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Preset timing and audio-reaction settings, plus which presets are used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectMSettings {
    /// Seconds each preset shows before switching to the next one.
    pub preset_duration_secs: f32,
    /// Seconds a soft (blended) transition between presets takes.
    pub soft_cut_secs: f32,
    /// Whether loud beats may trigger an immediate (hard) preset switch.
    pub hard_cuts: bool,
    /// How loud a beat must be to hard-cut; higher cuts less often.
    pub hard_cut_sensitivity: f32,
    /// How strongly presets react to beats.
    pub beat_sensitivity: f32,
    /// Random preset order instead of sequential.
    pub shuffle: bool,
    /// Stay on the current preset until unlocked.
    pub preset_locked: bool,
    /// Upper bound on rendered frames per second.
    pub fps_cap: u32,
    /// Preset packs (folder names under the presets directory) to leave out.
    /// Stored as the disabled set so packs installed later start enabled.
    pub disabled_packs: Vec<String>,
    /// An extra folder of the user's own presets, scanned recursively.
    pub user_preset_dir: Option<PathBuf>,
    /// The preset shown last, restored on start and after a surface change.
    pub last_preset: Option<PathBuf>,
}

/// Bounds applied by [`ProjectMSettings::sanitized`], so a hand-edited
/// config can't stall the renderer or divide by zero.
const DURATION_RANGE: (f32, f32) = (1.0, 3600.0);
const SOFT_CUT_RANGE: (f32, f32) = (0.0, 60.0);
const SENSITIVITY_RANGE: (f32, f32) = (0.0, 10.0);
const FPS_RANGE: (u32, u32) = (10, 240);

impl Default for ProjectMSettings {
    fn default() -> Self {
        Self {
            preset_duration_secs: 30.0,
            soft_cut_secs: 3.0,
            hard_cuts: false,
            hard_cut_sensitivity: 2.0,
            beat_sensitivity: 1.0,
            shuffle: true,
            preset_locked: false,
            fps_cap: 60,
            disabled_packs: Vec::new(),
            user_preset_dir: None,
            last_preset: None,
        }
    }
}

impl ProjectMSettings {
    /// A copy with every number clamped to a usable range (non-finite
    /// values fall back to the default).
    #[must_use]
    pub fn sanitized(&self) -> Self {
        let defaults = Self::default();
        let clamp = |value: f32, (min, max): (f32, f32), default: f32| {
            if value.is_finite() {
                value.clamp(min, max)
            } else {
                default
            }
        };
        Self {
            preset_duration_secs: clamp(
                self.preset_duration_secs,
                DURATION_RANGE,
                defaults.preset_duration_secs,
            ),
            soft_cut_secs: clamp(self.soft_cut_secs, SOFT_CUT_RANGE, defaults.soft_cut_secs),
            hard_cut_sensitivity: clamp(
                self.hard_cut_sensitivity,
                SENSITIVITY_RANGE,
                defaults.hard_cut_sensitivity,
            ),
            beat_sensitivity: clamp(
                self.beat_sensitivity,
                SENSITIVITY_RANGE,
                defaults.beat_sensitivity,
            ),
            fps_cap: self.fps_cap.clamp(FPS_RANGE.0, FPS_RANGE.1),
            ..self.clone()
        }
    }

    /// Whether presets from the pack folder `pack` are used.
    pub fn pack_enabled(&self, pack: &str) -> bool {
        !self.disabled_packs.iter().any(|disabled| disabled == pack)
    }
}
