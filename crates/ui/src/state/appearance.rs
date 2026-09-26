//! Appearance settings shared through the UI: the [`Theme`] colour scheme,
//! the [`Accent`] colour and the opaque [`Rgb`] building block, all free of
//! any GUI toolkit (the derived [`Palette`] lives in [`super::palette`]).

use serde::{Deserialize, Serialize};

/// Opaque sRGB colour, one byte per channel. The shared colour currency for
/// themes, accents and palettes; the app maps it to its own colour type
/// (`COLORREF`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red channel, 0–255.
    pub r: u8,
    /// Green channel, 0–255.
    pub g: u8,
    /// Blue channel, 0–255.
    pub b: u8,
}

impl Rgb {
    /// Opaque sRGB colour from its channels.
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// The channels as `[r, g, b]`.
    pub const fn to_array(self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }
}

/// MusicBee-ish orange, the default accent ([`Accent::Orange`]).
pub const DEFAULT_ACCENT: Rgb = Rgb::from_rgb(0xE8, 0x7A, 0x1E);

/// Default strength of the window accent tint (#355), `0..=255`, matching
/// `win32ui`'s own default.
pub const DEFAULT_ACCENT_TINT_STRENGTH: u8 = 0x66;

/// Colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    /// The other colour scheme.
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
    Custom(Rgb),
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
    pub fn rgb(self) -> Rgb {
        match self {
            Self::Orange => DEFAULT_ACCENT,
            Self::Blue => Rgb::from_rgb(0x35, 0x84, 0xE4),
            Self::Green => Rgb::from_rgb(0x2E, 0xC2, 0x7E),
            Self::Purple => Rgb::from_rgb(0x91, 0x41, 0xAC),
            Self::Red => Rgb::from_rgb(0xE0, 0x1B, 0x24),
            Self::Teal => Rgb::from_rgb(0x0F, 0x9B, 0xA0),
            Self::Custom(rgb) => rgb,
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
            Self::Custom(rgb) => {
                format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b)
            }
            other => other.label().to_ascii_lowercase(),
        }
    }
}

/// `#rrggbb` (with the `#` optional) to an opaque colour.
fn parse_hex(s: &str) -> Option<Rgb> {
    let trimmed = s.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    Some(Rgb::from_rgb(
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

#[cfg(test)]
mod tests {
    //! Unit tests for the [`Accent`] config/CLI spellings (preset names,
    //! `#rrggbb` custom colours, rejection of garbage), moved with the code
    //! (#94).

    use super::*;

    #[test]
    fn preset_names_parse_case_insensitively() {
        assert_eq!(Accent::parse("blue"), Some(Accent::Blue));
        assert_eq!(Accent::parse("PURPLE"), Some(Accent::Purple));
        assert_eq!(Accent::parse(" Teal "), Some(Accent::Teal));
        assert_eq!(Accent::parse("orange"), Some(Accent::Orange));
    }

    #[test]
    fn hex_colours_parse_to_custom() {
        assert_eq!(
            Accent::parse("#12abCF"),
            Some(Accent::Custom(Rgb::from_rgb(0x12, 0xAB, 0xCF)))
        );
        // The `#` is optional.
        assert_eq!(
            Accent::parse("e87a1e"),
            Some(Accent::Custom(Rgb::from_rgb(0xE8, 0x7A, 0x1E)))
        );
    }

    #[test]
    fn garbage_is_rejected() {
        assert_eq!(Accent::parse(""), None);
        assert_eq!(Accent::parse("magenta"), None);
        assert_eq!(Accent::parse("#12345"), None);
        assert_eq!(Accent::parse("#1234567"), None);
        assert_eq!(Accent::parse("#12g45z"), None);
    }

    #[test]
    fn config_spelling_round_trips_through_parse() {
        for accent in [
            Accent::Orange,
            Accent::Blue,
            Accent::Green,
            Accent::Purple,
            Accent::Red,
            Accent::Teal,
            Accent::Custom(Rgb::from_rgb(0xCA, 0xFE, 0xBA)),
        ] {
            assert_eq!(Accent::parse(&accent.to_config_str()), Some(accent));
        }
    }

    #[test]
    fn custom_colours_serialize_as_hex() {
        assert_eq!(
            Accent::Custom(Rgb::from_rgb(0xCA, 0xFE, 0xBA)).to_config_str(),
            "#cafeba"
        );
        assert_eq!(Accent::Blue.to_config_str(), "blue");
    }

    #[test]
    fn default_accent_is_orange() {
        assert_eq!(Accent::default().rgb(), DEFAULT_ACCENT);
        assert_eq!(DEFAULT_ACCENT, Rgb::from_rgb(0xE8, 0x7A, 0x1E));
    }
}
