//! [`VisualizerMode`]: which of the status-bar visualizer's modes (#25) is
//! active. Cycled by clicking the strip and persisted in the config.

use serde::{Deserialize, Serialize};

/// Visualizer strip mode (#25), cycled by clicking the strip and persisted
/// in the config.
///
/// Serialized with kebab-case names so the config file stays readable and
/// the same spelling works for `emusic-shot --visualizer`.
///
/// The projectM visualization has its own surfaces (#300), so the strip no
/// longer has a MilkDrop mode; configs that saved `milkdrop` load as
/// [`VisualizerMode::Spectrum`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VisualizerMode {
    /// 24–48 log-spaced bars from the channel's FFT magnitudes.
    #[default]
    #[serde(alias = "milkdrop")]
    Spectrum,
    /// The channel's raw float samples as a single trace.
    Oscilloscope,
    /// Nothing drawn; the strip reserves no repaints at all.
    Off,
}

impl VisualizerMode {
    pub const ALL: [Self; 3] = [Self::Spectrum, Self::Oscilloscope, Self::Off];

    /// Short label for the strip's tooltip / Settings.
    pub fn label(self) -> &'static str {
        match self {
            Self::Spectrum => "Spectrum",
            Self::Oscilloscope => "Oscilloscope",
            Self::Off => "Off",
        }
    }

    /// CLI-friendly identifier, e.g. for `emusic-shot --visualizer`.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Spectrum => "spectrum",
            Self::Oscilloscope => "oscilloscope",
            Self::Off => "off",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.slug() == slug)
    }

    /// The next mode in the click-to-cycle order.
    pub fn next(self) -> Self {
        match self {
            Self::Spectrum => Self::Oscilloscope,
            Self::Oscilloscope => Self::Off,
            Self::Off => Self::Spectrum,
        }
    }
}
