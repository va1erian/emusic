//! Single-line DirectWrite text shared by the Direct2D views (#235).
//!
//! GDI draws single-line text with `DT_SINGLELINE`/`DT_END_ELLIPSIS` and
//! `DT_VCENTER` in one call; DirectWrite lays out wrapped text and leaves
//! trimming and alignment to the caller, so these helpers elide to a width and
//! centre a line in its rectangle. They also convert the device-pixel `Rect`s
//! the shared layout modules produce into the device-independent [`RectF`]s
//! Direct2D draws in.

use std::cell::RefCell;
use std::collections::HashMap;

use win32ui::d2d::{D2dCanvas, Font, Layout, PointF, RectF};
use win32ui::{Color, Rect};

/// Horizontal alignment of one line within its rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    /// Start the line at the rectangle's left edge.
    Left,
    /// Centre the line horizontally in the rectangle.
    Center,
}

/// Colour and horizontal alignment of one line, grouped so the cached draw
/// path stays within clippy's argument limit.
#[derive(Clone, Copy)]
pub(crate) struct LineStyle {
    pub color: Color,
    pub align: Align,
}

impl LineStyle {
    /// A line drawn in `color`, starting at the rectangle's left edge.
    pub(crate) fn left(color: Color) -> Self {
        Self {
            color,
            align: Align::Left,
        }
    }
}

/// `text` shortened with a trailing ellipsis so it measures at most
/// `max_width`; returned unchanged when it already fits.
pub(crate) fn elide(font: &Font, text: &str, max_width: f32) -> String {
    elide_with(|text| font.width(text), text, max_width)
}

/// The string trimming behind [`elide`], over a caller-supplied width measure,
/// so the truncation can be unit-tested without a DirectWrite font.
fn elide_with(measure: impl Fn(&str) -> f32, text: &str, max_width: f32) -> String {
    if max_width <= 0.0 || measure(text) <= max_width {
        return text.to_owned();
    }
    const ELLIPSIS: &str = "\u{2026}";
    let tail = measure(ELLIPSIS);
    if tail > max_width {
        return String::new();
    }
    let mut end = text.len();
    while end > 0 {
        end -= 1;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        if measure(&text[..end]) + tail <= max_width {
            let mut trimmed = String::with_capacity(end + ELLIPSIS.len());
            trimmed.push_str(&text[..end]);
            trimmed.push_str(ELLIPSIS);
            return trimmed;
        }
    }
    ELLIPSIS.to_owned()
}

/// Draws `text` on one line, vertically centred in `rect` and elided to the
/// rectangle's width; `align` places the line horizontally.
pub(crate) fn draw_line(
    canvas: &mut D2dCanvas<'_>,
    font: &Font,
    rect: RectF,
    text: &str,
    color: Color,
    align: Align,
) {
    let text = elide(font, text, rect.width());
    let Ok(layout) = font.layout(&text, f32::INFINITY) else {
        return;
    };
    paint_layout(canvas, &layout, rect, LineStyle { color, align });
}

/// Paints an already-laid-out line, centred vertically in `rect`.
fn paint_layout(canvas: &mut D2dCanvas<'_>, layout: &Layout, rect: RectF, style: LineStyle) {
    let (width, height) = layout.size();
    let x = match style.align {
        Align::Left => rect.left,
        Align::Center => rect.left + (rect.width() - width) / 2.0,
    };
    let y = rect.top + (rect.height() - height) / 2.0;
    canvas.push_clip(rect);
    canvas.draw_text(layout, PointF::new(x, y), style.color);
    let _ = canvas.pop_clip();
}

/// How many laid-out lines one [`LineCache`] remembers before it is reset.
const LINE_CACHE_ENTRIES: usize = 2048;

/// A bounded cache of single-line [`Layout`]s, shared by a painter's fonts, so
/// a repaint of an unchanged caption reuses its DirectWrite layout instead of
/// creating a new one.
///
/// Keyed by a caller-assigned font role (so distinct fonts never collide), the
/// elided string and the available width. Reset by dropping it, which a
/// painter does when it rebuilds its fonts on an appearance change.
#[derive(Default)]
pub(crate) struct LineCache {
    entries: RefCell<HashMap<(u8, String, u32), Layout>>,
}

impl LineCache {
    /// Draws `text` exactly like [`draw_line`], reusing the layout when the
    /// same (font `role`, string, width) is drawn again.
    pub(crate) fn draw(
        &self,
        canvas: &mut D2dCanvas<'_>,
        role: u8,
        font: &Font,
        rect: RectF,
        text: &str,
        style: LineStyle,
    ) {
        let text = elide(font, text, rect.width());
        let key = (role, text, rect.width().to_bits());
        let mut entries = self.entries.borrow_mut();
        let layout = if let Some(layout) = entries.get(&key) {
            layout
        } else {
            let Ok(layout) = font.layout(&key.1, f32::INFINITY) else {
                return;
            };
            if entries.len() >= LINE_CACHE_ENTRIES {
                entries.clear();
            }
            entries.entry(key).or_insert(layout)
        };
        paint_layout(canvas, layout, rect, style);
    }
}

/// Converts a device-pixel rectangle to the device-independent rectangle
/// Direct2D draws in.
pub(crate) fn rect_to_dip(rect: Rect, scale: f32) -> RectF {
    RectF::new(
        rect.left as f32 / scale,
        rect.top as f32 / scale,
        rect.right as f32 / scale,
        rect.bottom as f32 / scale,
    )
}

/// Converts a device-independent rectangle to device pixels, rounding to the
/// nearest pixel.
pub(crate) fn rect_to_px(rect: RectF, scale: f32) -> Rect {
    Rect::new(
        (rect.left * scale).round() as i32,
        (rect.top * scale).round() as i32,
        (rect.right * scale).round() as i32,
        (rect.bottom * scale).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A measure that counts characters, so the expected widths are obvious.
    fn chars(text: &str) -> f32 {
        text.chars().count() as f32
    }

    #[test]
    fn elide_keeps_text_that_fits() {
        assert_eq!(elide_with(chars, "abc", 3.0), "abc");
        assert_eq!(elide_with(chars, "abc", 9.0), "abc");
    }

    #[test]
    fn elide_truncates_and_appends_an_ellipsis() {
        // Four cells for "abc…": three characters plus the one-cell ellipsis.
        assert_eq!(elide_with(chars, "abcdef", 4.0), "abc\u{2026}");
    }

    #[test]
    fn elide_shortens_at_char_boundaries() {
        // A three-byte character is dropped whole, not split.
        assert_eq!(
            elide_with(chars, "\u{4e16}\u{754c}xy", 3.0),
            "\u{4e16}\u{754c}\u{2026}"
        );
    }

    #[test]
    fn elide_gives_up_when_even_the_ellipsis_does_not_fit() {
        assert_eq!(elide_with(chars, "abcdef", 0.5), "");
    }

    #[test]
    fn rect_round_trips_through_dip() {
        let rect = Rect::new(3, 5, 103, 205);
        let dip = rect_to_dip(rect, 2.0);
        assert_eq!(rect_to_px(dip, 2.0), rect);
    }
}
