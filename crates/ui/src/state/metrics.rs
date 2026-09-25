//! Selectable UI font size, list density, zebra striping and the derived
//! [`Metrics`] (#309).
//!
//! [`FontSize`] is a stepped scale rather than a raw point size. The base
//! point size a given scale starts from is supplied by the caller
//! ([`Metrics::compute`] takes it as an argument). [`Density`] only pads rows,
//! never text. The app reads the resulting [`Metrics`] instead of computing
//! its own row heights.

use serde::{Deserialize, Serialize};

/// Stepped UI font scale, applied to every text style. Stored as an enum (not
/// points) so the scale derives identically from any base size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontSize {
    /// 90% of the base size.
    Small,
    /// 100% — the frontend's normal size.
    #[default]
    Default,
    /// 115% of the base size.
    Large,
    /// 130% of the base size.
    Larger,
}

impl FontSize {
    /// Every option, in display order.
    pub const ALL: [Self; 4] = [Self::Small, Self::Default, Self::Large, Self::Larger];

    /// Label shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Small",
            Self::Default => "Default",
            Self::Large => "Large",
            Self::Larger => "Larger",
        }
    }

    /// Multiplier applied to the frontend's base point size.
    pub fn scale(self) -> f32 {
        match self {
            Self::Small => 0.90,
            Self::Default => 1.0,
            Self::Large => 1.15,
            Self::Larger => 1.30,
        }
    }
}

/// Row padding multiplier: how airy list rows are, independent of the font
/// size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    /// Tight rows, for fitting as many entries on screen as possible.
    Compact,
    /// The default, MusicBee-like density.
    #[default]
    Comfortable,
    /// Roomier rows, easier to hit with a touch or an imprecise pointer.
    Spacious,
}

impl Density {
    /// Every option, in display order.
    pub const ALL: [Self; 3] = [Self::Compact, Self::Comfortable, Self::Spacious];

    /// Label shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Comfortable => "Comfortable",
            Self::Spacious => "Spacious",
        }
    }

    /// Multiplier applied to the row height and cell padding.
    pub fn row_scale(self) -> f32 {
        match self {
            Self::Compact => 0.85,
            Self::Comfortable => 1.0,
            Self::Spacious => 1.20,
        }
    }
}

/// The user's appearance choices beyond the theme and accent (#309), persisted
/// with the rest of the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Scale applied to every UI text style.
    pub font_size: FontSize,
    /// How much padding list rows get.
    pub density: Density,
    /// Whether list rows alternate their background (zebra striping).
    pub zebra: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_size: FontSize::default(),
            density: Density::default(),
            zebra: true,
        }
    }
}

/// Concrete sizes derived from [`Appearance`] for one frontend at a given DPI.
///
/// Row heights and padding are rounded to whole device pixels, so stripes stay
/// pixel-aligned (a main source of visible jank when rows land on fractional
/// heights). Text sizes stay continuous; only layout-affecting values are
/// snapped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Regular body text, in points.
    pub body: f32,
    /// Secondary/caption text, in points.
    pub small: f32,
    /// Section titles/headings, in points.
    pub title: f32,
    /// List row height, in points, for every list view.
    pub row_height: f32,
    /// List cell padding, in points.
    pub cell_padding: f32,
}

impl Metrics {
    /// Sizes at [`FontSize::Default`] and [`Density::Comfortable`] with a
    /// base body size of 13 points at 100% DPI.
    pub const DEFAULT: Self = Self {
        body: 13.0,
        small: 9.0,
        title: 18.0,
        row_height: 20.0,
        cell_padding: 4.0,
    };

    /// Smallest list row, so rows stay a usable click target even at the
    /// smallest font and tightest density.
    const MIN_ROW_HEIGHT: f32 = 16.0;

    /// Ratios of the base body size, keeping the relative type scale and row
    /// proportions fixed. The small ratio is the default caption size.
    const SMALL_RATIO: f32 = 9.0 / 13.0;
    const TITLE_RATIO: f32 = 18.0 / 13.0;
    const ROW_HEIGHT_RATIO: f32 = 20.0 / 13.0;
    const CELL_PADDING_RATIO: f32 = 4.0 / 13.0;

    /// Derives the concrete sizes from the appearance choices, the frontend's
    /// base body point size and its DPI scale (points per device pixel).
    pub fn compute(font_size: FontSize, density: Density, base_body_points: f32, dpi: f32) -> Self {
        let scale = font_size.scale();
        let pad_scale = density.row_scale();
        let body = base_body_points * scale;
        let row = (body * Self::ROW_HEIGHT_RATIO * pad_scale).max(Self::MIN_ROW_HEIGHT);
        Self {
            body,
            small: body * Self::SMALL_RATIO,
            title: body * Self::TITLE_RATIO,
            row_height: round_to_device(row, dpi),
            cell_padding: round_to_device(body * Self::CELL_PADDING_RATIO * pad_scale, dpi),
        }
    }
}

/// Rounds `points` so it lands on a whole device pixel at `dpi`, leaving
/// non-positive or non-finite scales untouched.
fn round_to_device(points: f32, dpi: f32) -> f32 {
    if !dpi.is_finite() || dpi <= 0.0 {
        return points;
    }
    (points * dpi).round() / dpi
}

#[cfg(test)]
mod tests {
    use super::*;

    const DPIS: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

    #[test]
    fn default_metrics_match_the_base_look() {
        let metrics = Metrics::compute(FontSize::Default, Density::Comfortable, 13.0, 1.0);
        assert_eq!(metrics, Metrics::DEFAULT);
    }

    #[test]
    fn row_heights_are_whole_device_pixels_at_every_dpi() {
        for font in FontSize::ALL {
            for density in Density::ALL {
                for dpi in DPIS {
                    let metrics = Metrics::compute(font, density, 13.0, dpi);
                    for (name, value) in [
                        ("row_height", metrics.row_height),
                        ("cell_padding", metrics.cell_padding),
                    ] {
                        let device = value * dpi;
                        assert!(
                            (device - device.round()).abs() < 1e-3,
                            "{font:?}/{density:?}@{dpi}: {name} = {value} is {device} device px"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn text_and_rows_grow_with_the_font_size() {
        let at = |font| Metrics::compute(font, Density::Comfortable, 13.0, 1.0);
        let sizes: Vec<Metrics> = FontSize::ALL.into_iter().map(at).collect();
        for pair in sizes.windows(2) {
            assert!(pair[0].body < pair[1].body, "body must grow");
            assert!(pair[0].row_height <= pair[1].row_height, "rows must grow");
        }
    }

    #[test]
    fn density_only_pads_rows_never_text() {
        let at = |density| Metrics::compute(FontSize::Default, density, 13.0, 1.0);
        let compact = at(Density::Compact);
        let comfortable = at(Density::Comfortable);
        let spacious = at(Density::Spacious);
        assert_eq!(compact.body, comfortable.body);
        assert_eq!(spacious.body, comfortable.body);
        assert!(compact.row_height < comfortable.row_height);
        assert!(comfortable.row_height < spacious.row_height);
        assert!(compact.cell_padding < spacious.cell_padding);
    }

    #[test]
    fn rows_stay_a_sane_minimum_click_target() {
        for font in FontSize::ALL {
            for density in Density::ALL {
                for dpi in DPIS {
                    let metrics = Metrics::compute(font, density, 13.0, dpi);
                    assert!(
                        metrics.row_height >= Metrics::MIN_ROW_HEIGHT,
                        "{font:?}/{density:?}@{dpi}: row is {}",
                        metrics.row_height
                    );
                    assert!(metrics.row_height > metrics.body);
                }
            }
        }
    }

    #[test]
    fn zero_or_invalid_dpi_is_left_untouched() {
        assert_eq!(round_to_device(17.3, 0.0), 17.3);
        assert_eq!(round_to_device(17.3, f32::NAN), 17.3);
    }
}
