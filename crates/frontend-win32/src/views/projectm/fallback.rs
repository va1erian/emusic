#![forbid(unsafe_code)]

//! The projectM placeholder (#301): a smooth audio-reactive plasma drawn with
//! GDI, plus a one-line hint, shown whenever the real engine cannot run. It
//! mirrors what `crates/app`'s projectM surfaces draw in the same situation
//! (see `emusic_milkdrop`).

use emusic_milkdrop::Frame;
use emusic_ui::state::projectm::ProjectMAvailability;
use win32ui::gdi::{Font, TextFormat};
use win32ui::{Color, Rect, Theme};

/// Plasma bands drawn vertically; enough to read as a gradient, few enough to
/// stay cheap at panel size.
const BANDS: usize = 48;
/// Height of the hint band at the bottom, in device pixels.
const HINT_HEIGHT: i32 = 22;

/// The one-line hint explaining why the placeholder is showing.
pub(crate) fn hint(availability: &ProjectMAvailability) -> &'static str {
    match availability {
        ProjectMAvailability::Unknown => "starting projectM\u{2026}",
        ProjectMAvailability::MissingLibrary => "projectM not installed",
        ProjectMAvailability::NoOpenGl => "OpenGL 3.3 unavailable",
        ProjectMAvailability::Available(_) => "",
    }
}

/// Draws `frame` as a vertical plasma, then `hint` on a band at the bottom.
/// `font` is reused across frames; when it is `None` the hint is skipped.
pub(crate) fn paint(
    canvas: &win32ui::gdi::Canvas,
    bounds: Rect,
    frame: Frame,
    theme: &Theme,
    font: Option<&Font>,
    hint: &str,
) {
    let height = (bounds.bottom - bounds.top).max(1);
    let bands = BANDS.min(height as usize);
    for band in 0..bands {
        let t = band as f32 / (bands.max(2) - 1) as f32;
        let hue = (frame.hue + t * 0.35 + frame.swirl * 0.05).rem_euclid(1.0);
        let wobble = (t * std::f32::consts::TAU + frame.swirl).sin() * 0.5 + 0.5;
        // Audible: the pulse brightens every band. Silent: a mid-bright base
        // keeps the gradient readable without looking like a black rectangle.
        let value = ((0.45 + 0.55 * frame.pulse) * (0.6 + 0.4 * wobble)).clamp(0.0, 1.0);
        let top = bounds.top + band as i32 * height / bands as i32;
        let bottom = bounds.top + (band as i32 + 1) * height / bands as i32;
        canvas.fill_rect(
            Rect::new(bounds.left, top, bounds.right, bottom.max(top + 1)),
            hsv(hue, 0.6, value),
        );
    }

    let Some(font) = font else {
        return;
    };
    if hint.is_empty() {
        return;
    }
    let bar = Rect::new(
        bounds.left,
        (bounds.bottom - HINT_HEIGHT).max(bounds.top),
        bounds.right,
        bounds.bottom,
    );
    canvas.fill_rect(bar, theme.input_background);
    let format = TextFormat::left()
        .center()
        .vcenter()
        .single_line()
        .end_ellipsis()
        .no_prefix();
    canvas.with_font(font, |canvas| {
        canvas.draw_text(bar, hint, theme.text, format);
    });
}

/// A hue/saturation/value colour, each component clamped where sensible.
fn hsv(hue: f32, saturation: f32, value: f32) -> Color {
    let hue = hue.rem_euclid(1.0) * 6.0;
    let sector = hue.floor();
    let fraction = hue - sector;
    let saturation = saturation.clamp(0.0, 1.0);
    let value = value.clamp(0.0, 1.0);
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - saturation * fraction);
    let t = value * (1.0 - saturation * (1.0 - fraction));
    let (r, g, b) = match sector as i32 % 6 {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        _ => (value, p, q),
    };
    let channel = |component: f32| (component * 255.0).round().clamp(0.0, 255.0) as u8;
    Color::rgb(channel(r), channel(g), channel(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_covers_every_unavailable_state() {
        assert_eq!(
            hint(&ProjectMAvailability::MissingLibrary),
            "projectM not installed"
        );
        assert_eq!(
            hint(&ProjectMAvailability::NoOpenGl),
            "OpenGL 3.3 unavailable"
        );
        assert!(hint(&ProjectMAvailability::Available("4.1.7".into())).is_empty());
    }

    #[test]
    fn hsv_stays_in_range_for_out_of_range_inputs() {
        for hue in [-2.0f32, -0.1, 0.0, 0.5, 1.0, 7.3] {
            for saturation in [-1.0f32, 0.0, 0.5, 1.0, 2.0] {
                for value in [-1.0f32, 0.0, 0.5, 1.0, 2.0] {
                    let _ = hsv(hue, saturation, value);
                }
            }
        }
        assert_eq!(hsv(0.0, 0.0, 1.0), Color::rgb(255, 255, 255));
        assert_eq!(hsv(0.0, 0.0, 0.0), Color::rgb(0, 0, 0));
        assert_eq!(hsv(0.0, 1.0, 1.0), Color::rgb(255, 0, 0));
    }
}
