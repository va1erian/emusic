#![forbid(unsafe_code)]

//! SID playback settings and their three-tier resolution.
//!
//! Mirrors [`crate::tracker`]: a global [`SidSettings`], optional per-format
//! overrides (the `.sid` / `.psid` / `.rsid` extensions) and optional per-file
//! overrides, resolved per-file → per-format → global.
//!
//! Chip model and clock are load-time only in cRSID (they're applied between
//! processing the tune and running its init routine), so a change takes effect
//! the next time a SID file is opened.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Default tune length when neither HVSC nor a setting provides one.
pub const DEFAULT_SONG_LENGTH_SECS: u32 = 180;

/// SID chip model override; `Auto` follows the file's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidChipModel {
    #[default]
    Auto,
    Mos6581,
    Mos8580,
}

impl SidChipModel {
    /// The engine override, or `None` to follow the file.
    pub fn to_engine(self) -> Option<emusic_sid::ChipModel> {
        match self {
            Self::Auto => None,
            Self::Mos6581 => Some(emusic_sid::ChipModel::Mos6581),
            Self::Mos8580 => Some(emusic_sid::ChipModel::Mos8580),
        }
    }

    /// A short human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (header)",
            Self::Mos6581 => "6581",
            Self::Mos8580 => "8580",
        }
    }
}

/// Clock override; `Auto` follows the file's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidClock {
    #[default]
    Auto,
    Pal,
    Ntsc,
}

impl SidClock {
    /// The engine override, or `None` to follow the file.
    pub fn to_engine(self) -> Option<emusic_sid::Clock> {
        match self {
            Self::Auto => None,
            Self::Pal => Some(emusic_sid::Clock::Pal),
            Self::Ntsc => Some(emusic_sid::Clock::Ntsc),
        }
    }

    /// A short human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (header)",
            Self::Pal => "PAL",
            Self::Ntsc => "NTSC",
        }
    }
}

/// The three SID file extensions the per-format tier can address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidFormat {
    Sid,
    Psid,
    Rsid,
}

impl SidFormat {
    /// Every format, for UI iteration.
    pub const ALL: [Self; 3] = [Self::Sid, Self::Psid, Self::Rsid];

    /// The format named by `path`'s extension, if any.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("sid") => Some(Self::Sid),
            Some("psid") => Some(Self::Psid),
            Some("rsid") => Some(Self::Rsid),
            _ => None,
        }
    }

    /// The extension this format represents.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Sid => "sid",
            Self::Psid => "psid",
            Self::Rsid => "rsid",
        }
    }
}

/// The SID settings for one tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidSettings {
    /// Chip model override.
    pub chip_model: SidChipModel,
    /// Clock override.
    pub clock: SidClock,
    /// Tune length used when HVSC has no song length for the tune.
    pub default_song_length_secs: u32,
}

impl Default for SidSettings {
    fn default() -> Self {
        Self {
            chip_model: SidChipModel::Auto,
            clock: SidClock::Auto,
            default_song_length_secs: DEFAULT_SONG_LENGTH_SECS,
        }
    }
}

impl SidSettings {
    /// Clamps numeric fields into range.
    pub fn sanitize(&mut self) {
        self.default_song_length_secs = self.default_song_length_secs.clamp(1, 3600);
    }

    /// The fallback tune length.
    pub fn default_length(self) -> Duration {
        Duration::from_secs(u64::from(self.default_song_length_secs))
    }

    /// The engine config implied by these settings.
    pub fn to_engine_config(self) -> emusic_sid::SidConfig {
        emusic_sid::SidConfig {
            chip_model: self.chip_model.to_engine(),
            clock: self.clock.to_engine(),
            subtune: None,
        }
    }
}

/// Global + per-format + per-file SID settings, resolved per-file →
/// per-format → global.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SidConfig {
    /// Settings used when no override matches.
    pub global: SidSettings,
    /// Overrides keyed by file extension.
    pub per_format: HashMap<SidFormat, SidSettings>,
    /// Overrides keyed by the full path string.
    pub per_file: HashMap<String, SidSettings>,
}

impl SidConfig {
    /// A config with the given global settings and no overrides.
    pub fn new(global: SidSettings) -> Self {
        Self {
            global,
            ..Default::default()
        }
    }

    /// Resolves the settings for `path`, most specific first.
    pub fn resolve(&self, path: &Path) -> SidSettings {
        if let Some(settings) = self.per_file.get(path.as_os_str().to_str().unwrap_or("")) {
            return *settings;
        }
        if let Some(format) = SidFormat::from_path(path)
            && let Some(settings) = self.per_format.get(&format)
        {
            return *settings;
        }
        self.global
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_prefers_file_then_format_then_global() {
        let global = SidSettings {
            chip_model: SidChipModel::Auto,
            ..SidSettings::default()
        };
        let format = SidSettings {
            chip_model: SidChipModel::Mos8580,
            ..SidSettings::default()
        };
        let file = SidSettings {
            chip_model: SidChipModel::Mos6581,
            ..SidSettings::default()
        };

        let mut config = SidConfig::new(global);
        assert_eq!(config.resolve(Path::new("a.sid")).chip_model, SidChipModel::Auto);

        config.per_format.insert(SidFormat::Sid, format);
        assert_eq!(
            config.resolve(Path::new("a.sid")).chip_model,
            SidChipModel::Mos8580
        );
        assert_eq!(
            config.resolve(Path::new("a.xm")).chip_model,
            SidChipModel::Auto
        );

        config.per_file.insert("a.sid".to_string(), file);
        assert_eq!(
            config.resolve(Path::new("a.sid")).chip_model,
            SidChipModel::Mos6581
        );
    }

    #[test]
    fn sanitize_clamps_the_default_length() {
        let mut settings = SidSettings {
            default_song_length_secs: 0,
            ..SidSettings::default()
        };
        settings.sanitize();
        assert_eq!(settings.default_song_length_secs, 1);
    }

    #[test]
    fn settings_round_trip_through_toml() {
        let mut config = SidConfig::new(SidSettings::default());
        config.per_format.insert(
            SidFormat::Rsid,
            SidSettings {
                clock: SidClock::Ntsc,
                ..SidSettings::default()
            },
        );
        let text = toml::to_string(&config).expect("serialize");
        let back: SidConfig = toml::from_str(&text).expect("deserialize");
        assert_eq!(config, back);
    }

    #[test]
    fn engine_overrides_map_from_auto() {
        assert_eq!(SidChipModel::Auto.to_engine(), None);
        assert_eq!(
            SidChipModel::Mos8580.to_engine(),
            Some(emusic_sid::ChipModel::Mos8580)
        );
        assert_eq!(SidClock::Auto.to_engine(), None);
        assert_eq!(SidClock::Ntsc.to_engine(), Some(emusic_sid::Clock::Ntsc));
    }
}
