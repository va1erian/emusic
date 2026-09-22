//! Tracker module playback settings and named presets.

use serde::{Deserialize, Serialize};

/// Sample interpolation mode for tracker module mixing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Interpolation {
    /// No interpolation (harsh, "chip" sound).
    None,
    /// Linear interpolation (BASS default).
    #[default]
    Linear,
    /// Sinc interpolation (highest quality).
    Sinc,
}

/// Volume/panning ramping mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Ramping {
    /// No ramping.
    Off,
    /// Normal ramping.
    #[default]
    Normal,
    /// Sensitive ramping, closer to a tracker's own behaviour.
    Sensitive,
}

/// Surround sound processing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Surround {
    /// Off.
    #[default]
    Off,
    /// Surround mode 1.
    Mode1,
    /// Surround mode 2.
    Mode2,
}

/// Tracker emulation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Emulation {
    /// Let BASS decide based on the file type.
    #[default]
    Auto,
    /// FastTracker 2 style.
    Ft2,
    /// ProTracker 1 style.
    Pt1,
}

/// What to do when the module reaches its nominal end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EndBehavior {
    /// Stop playback.
    #[default]
    StopAtEnd,
    /// Follow the module's internal loop/backward-jump commands.
    FollowLoops,
    /// Loop the whole module this many times.
    ///
    /// Note: BASS only supports on/off looping via flags. Exact loop counts
    /// require sync callbacks and are left as a follow-up; this value is
    /// stored and mapped to the `BASS_MUSIC_LOOP` flag when non-zero.
    LoopTimes(u8),
}

/// Playback settings for a tracker module (MOD/S3M/XM/IT/MTM/UMX/MO3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerSettings {
    /// Sample interpolation mode.
    pub interpolation: Interpolation,
    /// Volume/pan ramping mode.
    pub ramping: Ramping,
    /// Stereo separation, `0..=100` (`100` = full separation, BASS default).
    pub stereo_separation: u8,
    /// Amplification, `0..=100` (`50` = BASS default).
    pub amplify: u8,
    /// Surround mode.
    pub surround: Surround,
    /// Tracker emulation mode.
    pub emulation: Emulation,
    /// Apply FastTracker 2's 100% panning (MOD files; same flag bit as FT2
    /// emulation on XM/MOD).
    pub ft2_pan: bool,
    /// End-of-module behaviour.
    pub end: EndBehavior,
    /// Global `BASS_CONFIG_SRC` resampler quality, `0..=4`.
    pub resampling_quality: u8,
}

impl Default for TrackerSettings {
    /// Matches BASS's built-in defaults.
    fn default() -> Self {
        Self {
            interpolation: Interpolation::Linear,
            ramping: Ramping::Off,
            stereo_separation: 100,
            amplify: 50,
            surround: Surround::Off,
            emulation: Emulation::Auto,
            ft2_pan: false,
            end: EndBehavior::StopAtEnd,
            resampling_quality: 2,
        }
    }
}

impl TrackerSettings {
    /// BASS defaults: linear interpolation, no ramping, full separation.
    pub const fn default_preset() -> Self {
        Self {
            interpolation: Interpolation::Linear,
            ramping: Ramping::Off,
            stereo_separation: 100,
            amplify: 50,
            surround: Surround::Off,
            emulation: Emulation::Auto,
            ft2_pan: false,
            end: EndBehavior::StopAtEnd,
            resampling_quality: 2,
        }
    }

    /// Amiga authentic: no interpolation, ProTracker 1 emulation, ramping off.
    pub const fn amiga_authentic() -> Self {
        Self {
            interpolation: Interpolation::None,
            ramping: Ramping::Off,
            stereo_separation: 100,
            amplify: 50,
            surround: Surround::Off,
            emulation: Emulation::Pt1,
            ft2_pan: false,
            end: EndBehavior::StopAtEnd,
            resampling_quality: 0,
        }
    }

    /// Smooth: sinc interpolation, sensitive ramping, half separation.
    pub const fn smooth() -> Self {
        Self {
            interpolation: Interpolation::Sinc,
            ramping: Ramping::Sensitive,
            stereo_separation: 50,
            amplify: 50,
            surround: Surround::Off,
            emulation: Emulation::Auto,
            ft2_pan: false,
            end: EndBehavior::StopAtEnd,
            resampling_quality: 4,
        }
    }

    /// Clamps numeric fields to their valid ranges.
    pub fn sanitize(&mut self) {
        self.stereo_separation = self.stereo_separation.clamp(0, 100);
        self.amplify = self.amplify.clamp(0, 100);
        self.resampling_quality = self.resampling_quality.clamp(0, 4);
    }
}
