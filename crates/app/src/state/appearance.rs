//! Appearance settings shared through the UI: the [`Theme`] colour scheme and
//! the [`Accent`] colour.

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

/// Colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

/// The UI accent colour: a built-in preset or a custom colour picked in
/// Settings → Appearance (#40).
///
/// Serializes as the preset's lowercase name (`"blue"`) or a `#rrggbb`
/// string for custom colours, so the config file stays human-editable and
/// the same spellings work for `emusic-shot --accent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Accent {
    #[default]
    Orange,
    Blue,
    Green,
    Purple,
    Red,
    Teal,
    /// Any colour chosen with the custom picker.
    Custom(Color32),
}

impl Accent {
    /// The built-in presets, in display order.
    pub const PRESETS: [Self; 6] = [
        Self::Orange,
        Self::Blue,
        Self::Green,
        Self::Purple,
        Self::Red,
        Self::Teal,
    ];

    /// Label shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Orange => "Orange",
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Purple => "Purple",
            Self::Red => "Red",
            Self::Teal => "Teal",
            Self::Custom(_) => "Custom",
        }
    }

    /// The colour to render with.
    pub fn color(self) -> Color32 {
        match self {
            Self::Orange => crate::theme::DEFAULT_ACCENT,
            Self::Blue => Color32::from_rgb(0x35, 0x84, 0xE4),
            Self::Green => Color32::from_rgb(0x2E, 0xC2, 0x7E),
            Self::Purple => Color32::from_rgb(0x91, 0x41, 0xAC),
            Self::Red => Color32::from_rgb(0xE0, 0x1B, 0x24),
            Self::Teal => Color32::from_rgb(0x0F, 0x9B, 0xA0),
            Self::Custom(color) => color,
        }
    }

    /// Parses the config/CLI spelling: a preset name (case-insensitive) or
    /// a `#rrggbb` hex colour.
    pub fn parse(s: &str) -> Option<Self> {
        let lowered = s.trim().to_ascii_lowercase();
        if let Some(preset) = Self::PRESETS
            .into_iter()
            .find(|preset| preset.label().to_ascii_lowercase() == lowered)
        {
            return Some(preset);
        }
        parse_hex(s).map(Self::Custom)
    }

    /// The canonical config/CLI spelling. [`Self::parse`] also accepts
    /// preset names in any case.
    pub fn to_config_str(self) -> String {
        match self {
            Self::Custom(color) => {
                format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
            }
            other => other.label().to_ascii_lowercase(),
        }
    }
}

/// `#rrggbb` (with the `#` optional) to an opaque colour.
fn parse_hex(s: &str) -> Option<Color32> {
    let trimmed = s.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    Some(Color32::from_rgb(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    ))
}

impl Serialize for Accent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_config_str())
    }
}

impl<'de> Deserialize<'de> for Accent {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown accent {s:?}; expected a preset name or #rrggbb"
            ))
        })
    }
}
