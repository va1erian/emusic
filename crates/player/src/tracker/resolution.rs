//! Per-file / per-format / global profile resolution for tracker settings.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::settings::TrackerSettings;

/// A tracker module file format that BASS can load via `BASS_MusicLoad`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackerFormat {
    /// ProTracker / NoiseTracker module.
    Mod,
    /// Scream Tracker 3 module.
    S3m,
    /// FastTracker 2 module.
    Xm,
    /// Impulse Tracker module.
    It,
    /// MultiTracker module.
    Mtm,
    /// Unreal Music Package.
    Umx,
    /// MO3 compressed module.
    Mo3,
}

impl TrackerFormat {
    /// Recognises a tracker format from a file extension, case-insensitively.
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .and_then(|ext| match ext.as_str() {
                "mod" => Some(Self::Mod),
                "s3m" => Some(Self::S3m),
                "xm" => Some(Self::Xm),
                "it" => Some(Self::It),
                "mtm" => Some(Self::Mtm),
                "umx" => Some(Self::Umx),
                "mo3" => Some(Self::Mo3),
                _ => None,
            })
    }
}

/// Collection of tracker settings at three resolution levels.
///
/// Resolution order for a given file is:
/// 1. Per-file override (matched by full path).
/// 2. Per-format override (matched by extension).
/// 3. Global profile.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackerConfig {
    /// Fallback settings used when no override matches.
    pub global: TrackerSettings,
    /// Overrides keyed by module format.
    pub per_format: HashMap<TrackerFormat, TrackerSettings>,
    /// Overrides keyed by full file path.
    pub per_file: HashMap<String, TrackerSettings>,
}

impl TrackerConfig {
    /// Creates a config with the given global profile and empty overrides.
    pub fn new(global: TrackerSettings) -> Self {
        Self {
            global,
            ..Default::default()
        }
    }

    /// Resolves the effective settings for `path`.
    ///
    /// Per-file wins over per-format, which wins over the global profile.
    pub fn resolve(&self, path: &Path) -> TrackerSettings {
        if let Some(settings) = self.per_file.get(path.as_os_str().to_str().unwrap_or("")) {
            return *settings;
        }
        if let Some(format) = TrackerFormat::from_path(path)
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
    use crate::tracker::settings::{Emulation, Interpolation, TrackerSettings};

    fn amiga() -> TrackerSettings {
        TrackerSettings::amiga_authentic()
    }

    fn smooth() -> TrackerSettings {
        TrackerSettings::smooth()
    }

    #[test]
    fn format_recognised_from_extension() {
        assert_eq!(
            TrackerFormat::from_path(Path::new("song.mod")),
            Some(TrackerFormat::Mod)
        );
        assert_eq!(
            TrackerFormat::from_path(Path::new("song.s3m")),
            Some(TrackerFormat::S3m)
        );
        assert_eq!(
            TrackerFormat::from_path(Path::new("song.xm")),
            Some(TrackerFormat::Xm)
        );
        assert_eq!(
            TrackerFormat::from_path(Path::new("song.it")),
            Some(TrackerFormat::It)
        );
        assert_eq!(
            TrackerFormat::from_path(Path::new("song.mtm")),
            Some(TrackerFormat::Mtm)
        );
        assert_eq!(
            TrackerFormat::from_path(Path::new("song.umx")),
            Some(TrackerFormat::Umx)
        );
        assert_eq!(
            TrackerFormat::from_path(Path::new("song.mo3")),
            Some(TrackerFormat::Mo3)
        );
    }

    #[test]
    fn format_detection_is_case_insensitive() {
        assert_eq!(
            TrackerFormat::from_path(Path::new("song.MOD")),
            Some(TrackerFormat::Mod)
        );
        assert_eq!(
            TrackerFormat::from_path(Path::new("song.Xm")),
            Some(TrackerFormat::Xm)
        );
    }

    #[test]
    fn non_tracker_extension_returns_none() {
        assert_eq!(TrackerFormat::from_path(Path::new("song.mp3")), None);
        assert_eq!(TrackerFormat::from_path(Path::new("no_extension")), None);
    }

    #[test]
    fn resolve_uses_global_by_default() {
        let global = amiga();
        let config = TrackerConfig::new(global);
        assert_eq!(config.resolve(Path::new("song.mod")), global);
        assert_eq!(config.resolve(Path::new("song.mp3")), global);
    }

    #[test]
    fn per_format_overrides_global() {
        let mut config = TrackerConfig::new(amiga());
        config.per_format.insert(TrackerFormat::Mod, smooth());

        assert_eq!(config.resolve(Path::new("a.mod")), smooth());
        assert_eq!(config.resolve(Path::new("a.xm")), amiga());
    }

    #[test]
    fn per_file_overrides_everything() {
        let mut config = TrackerConfig::new(amiga());
        config.per_format.insert(TrackerFormat::Mod, smooth());
        let file_override = TrackerSettings {
            interpolation: Interpolation::Sinc,
            emulation: Emulation::Pt1,
            ..TrackerSettings::default()
        };
        config
            .per_file
            .insert("special.mod".to_string(), file_override);

        assert_eq!(config.resolve(Path::new("special.mod")), file_override);
        assert_eq!(config.resolve(Path::new("other.mod")), smooth());
        assert_eq!(config.resolve(Path::new("song.xm")), amiga());
    }
}
