//! Rasterises the taskbar buttons' transport glyphs into in-memory Windows
//! icon resources.
//!
//! The shell's `THUMBBUTTON`s need `HICON`s, and the app has no transport
//! icon assets of its own (the egui UI paints its glyphs). Instead of
//! committing four binary icons, the glyphs are rasterised here, with 4x
//! supersampling, into the `BITMAPINFOHEADER` + 32bpp XOR + 1bpp AND layout
//! [`CreateIconFromResourceEx`] consumes. Keeping this pure Rust and
//! platform-independent means it is unit-testable without touching Win32.
//!
//! [`CreateIconFromResourceEx`]: windows::Win32::UI::WindowsAndMessaging::CreateIconFromResourceEx

/// Edge length, in pixels, of every generated icon.
const SIZE: u32 = 32;

/// Number of samples taken along each axis per pixel; 4 gives 16 samples per
/// pixel, enough to smooth the diagonals without a visible cost.
const SUPERSAMPLE: u32 = 4;

/// Size of the `BITMAPINFOHEADER` that prefixes an icon resource.
const HEADER_LEN: usize = 40;

/// Bytes per row of the 1bpp AND mask, padded to a 32-bit boundary.
const AND_ROW_LEN: usize = (SIZE as usize).div_ceil(32) * 4;

/// One of the four transport glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Glyph {
    Previous,
    Play,
    Pause,
    Next,
}

/// A filled polygon in normalised `[0, 1]` icon coordinates.
type Poly = &'static [(f32, f32)];

/// Previous: a bar next to a left-pointing triangle. Proportions match the
/// egui transport icons (`crate::icons`).
const PREVIOUS: &[Poly] = &[
    &[(0.21, 0.21), (0.3028, 0.21), (0.3028, 0.79), (0.21, 0.79)],
    &[(0.79, 0.21), (0.79, 0.79), (0.3376, 0.5)],
];

/// Next: the horizontal mirror of [`PREVIOUS`].
const NEXT: &[Poly] = &[
    &[(0.6972, 0.21), (0.79, 0.21), (0.79, 0.79), (0.6972, 0.79)],
    &[(0.21, 0.21), (0.21, 0.79), (0.6624, 0.5)],
];

/// Play: a right-pointing triangle, nudged right to look optically centred.
const PLAY: &[Poly] = &[&[(0.2872, 0.22), (0.8472, 0.5), (0.2872, 0.78)]];

/// Pause: two full-height vertical bars with a gap between them.
const PAUSE: &[Poly] = &[
    &[(0.25, 0.25), (0.40, 0.25), (0.40, 0.75), (0.25, 0.75)],
    &[(0.60, 0.25), (0.75, 0.25), (0.75, 0.75), (0.60, 0.75)],
];

/// The filled polygons making up `glyph`.
fn polygons(glyph: Glyph) -> &'static [Poly] {
    match glyph {
        Glyph::Previous => PREVIOUS,
        Glyph::Play => PLAY,
        Glyph::Pause => PAUSE,
        Glyph::Next => NEXT,
    }
}

/// Builds the icon resource [`CreateIconFromResourceEx`] needs: a
/// `BITMAPINFOHEADER` followed by a bottom-up 32bpp BGRA XOR bitmap and an
/// all-zero 1bpp AND mask. The glyph is painted white; its alpha channel
/// carries the shape, so the taskbar composites it over whatever background
/// the thumbnail preview uses.
///
/// [`CreateIconFromResourceEx`]: windows::Win32::UI::WindowsAndMessaging::CreateIconFromResourceEx
pub(super) fn resource(glyph: Glyph) -> Vec<u8> {
    let alpha = coverage(polygons(glyph));
    let pixels = (SIZE * SIZE) as usize;
    let mut data = Vec::with_capacity(HEADER_LEN + pixels * 4 + AND_ROW_LEN * SIZE as usize);

    // BITMAPINFOHEADER. `biHeight` counts both bitmaps, per the icon format.
    push_u32(&mut data, HEADER_LEN as u32);
    push_u32(&mut data, SIZE);
    push_u32(&mut data, SIZE * 2);
    push_u16(&mut data, 1);
    push_u16(&mut data, 32);
    push_u32(&mut data, 0); // BI_RGB
    push_u32(&mut data, 0);
    push_u32(&mut data, 0);
    push_u32(&mut data, 0);
    push_u32(&mut data, 0);
    push_u32(&mut data, 0);

    // XOR bitmap: bottom-up, BGRA, white with the coverage as alpha.
    for y in (0..SIZE as usize).rev() {
        for x in 0..SIZE as usize {
            let alpha = alpha[y * SIZE as usize + x];
            data.extend_from_slice(&[0xff, 0xff, 0xff, alpha]);
        }
    }

    // AND mask: opaque everywhere; the alpha channel already shapes the icon.
    data.resize(data.len() + AND_ROW_LEN * SIZE as usize, 0);
    data
}

/// Per-pixel coverage (0-255) of the union of `polygons`, sampled
/// `SUPERSAMPLE` times per axis.
fn coverage(polygons: &[Poly]) -> Vec<u8> {
    let side = SIZE as usize;
    let mut out = vec![0u8; side * side];
    let samples = SUPERSAMPLE * SUPERSAMPLE;
    for py in 0..side {
        for px in 0..side {
            let mut hits = 0u32;
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let x = (px as f32 + (sx as f32 + 0.5) / SUPERSAMPLE as f32) / SIZE as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / SUPERSAMPLE as f32) / SIZE as f32;
                    if polygons.iter().any(|poly| inside(poly, x, y)) {
                        hits += 1;
                    }
                }
            }
            out[py * side + px] = (hits * 255 / samples) as u8;
        }
    }
    out
}

/// Whether `(x, y)` is inside `poly`, by counting edge crossings of a ray
/// cast to the right. Works for any simple polygon, concave included.
fn inside(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn push_u16(data: &mut Vec<u8>, value: u16) {
    data.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(data: &mut Vec<u8>, value: u32) {
    data.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    const XOR_LEN: usize = (SIZE * SIZE * 4) as usize;
    const AND_LEN: usize = AND_ROW_LEN * SIZE as usize;

    fn alpha_at(glyph: Glyph, x: usize, y: usize) -> u8 {
        coverage(polygons(glyph))[y * SIZE as usize + x]
    }

    #[test]
    fn resource_has_the_icon_header_and_both_bitmaps() {
        let data = resource(Glyph::Play);
        assert_eq!(data.len(), HEADER_LEN + XOR_LEN + AND_LEN);
        let u32_at =
            |offset: usize| u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        assert_eq!(u32_at(0), HEADER_LEN as u32);
        assert_eq!(u32_at(4), SIZE);
        assert_eq!(u32_at(8), SIZE * 2, "height covers XOR and AND");
        assert_eq!(u16::from_le_bytes(data[12..14].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(data[14..16].try_into().unwrap()), 32);
    }

    #[test]
    fn glyphs_fill_their_centres_and_leave_the_corners_empty() {
        for glyph in [Glyph::Previous, Glyph::Play, Glyph::Pause, Glyph::Next] {
            // The pause centre sits in the gap between its two bars.
            let centre = if matches!(glyph, Glyph::Pause) {
                0
            } else {
                255
            };
            assert_eq!(alpha_at(glyph, 16, 16), centre, "{glyph:?} centre");
            let last = SIZE as usize - 1;
            assert_eq!(alpha_at(glyph, 0, 0), 0, "{glyph:?} top-left");
            assert_eq!(alpha_at(glyph, last, 0), 0, "{glyph:?} top-right");
            assert_eq!(alpha_at(glyph, 0, last), 0, "{glyph:?} bottom-left");
        }
    }

    #[test]
    fn previous_and_next_are_horizontal_mirrors() {
        let side = SIZE as usize;
        for y in 0..side {
            for x in 0..side {
                assert_eq!(
                    alpha_at(Glyph::Previous, x, y),
                    alpha_at(Glyph::Next, side - 1 - x, y),
                    "mirror differs at {x},{y}"
                );
            }
        }
    }
}
