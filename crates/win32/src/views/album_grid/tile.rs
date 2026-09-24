//! The album grid's tiles (#113): the model item, its [`GridModel`] wrapper and
//! the `content` painter that draws a cover, its caption and the selection.
//!
//! The painter receives a [`Canvas`] and the tile rectangle from the shared
//! [`GridView`](win32ui::GridView); it draws the cached cover (or a
//! deterministic placeholder) and the album/artist/year caption. Cover colour
//! and caption formatting mirror the egui frontend's `album_grid::tile`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use emusic_ui::views::album_grid::models::AlbumKey;
use win32ui::gdi::{Canvas, Font, FontWeight, TextFormat};
use win32ui::prelude::*;

use super::thumbs::ThumbState;

/// Caption height reserved under the cover, in design units (matches egui).
pub(super) const CAPTION_DIP: f32 = 42.0;

/// One album tile: the identity used for messages plus the caption fields and
/// the source path its cover is read from.
pub(super) struct AlbumTile {
    pub(super) key: AlbumKey,
    pub(super) name: String,
    pub(super) artist: String,
    pub(super) year: Option<u32>,
    pub(super) art_path: String,
}

/// The grid's owner-data model: the albums in display order.
pub(super) struct TileModel {
    pub(super) tiles: Rc<Vec<AlbumTile>>,
}

impl GridModel for TileModel {
    type Item = AlbumTile;

    fn len(&self) -> usize {
        self.tiles.len()
    }

    fn get(&self, index: usize) -> Option<&AlbumTile> {
        self.tiles.as_slice().get(index)
    }
}

/// Builds the `content` painter, capturing the shared thumbnail state, the
/// current theme and the fonts it draws with.
pub(super) fn content(
    thumbs: Rc<RefCell<ThumbState>>,
    theme: Rc<Cell<Theme>>,
    dpi: u32,
    caption_px: i32,
) -> impl Fn(&AlbumTile, &Canvas, Rect, TileState) + 'static {
    let body = Font::new("Segoe UI", 9.75, FontWeight::Regular, dpi).ok();
    let bold = Font::new("Segoe UI", 9.75, FontWeight::Bold, dpi).ok();
    let small = Font::new("Segoe UI", 8.25, FontWeight::Regular, dpi).ok();
    let glyph = Font::new("Segoe UI Symbol", 22.0, FontWeight::Regular, dpi).ok();

    move |tile, canvas, rect, state| {
        let theme = theme.get();
        let edge = (rect.width() - caption_px).max(1);
        let cover = Rect::new(rect.left, rect.top, rect.left + edge, rect.top + edge);

        match thumbs.borrow_mut().cover(&tile.art_path) {
            Some(bitmap) => canvas.draw_bitmap(&bitmap, cover),
            None => {
                canvas.fill_rect(cover, placeholder_color(&tile.name));
                if let Some(font) = &glyph {
                    canvas.with_font(font, |canvas| {
                        canvas.draw_text(
                            cover,
                            "\u{266A}",
                            Color::rgb(255, 255, 255),
                            TextFormat::left()
                                .center()
                                .vcenter()
                                .single_line()
                                .no_prefix(),
                        );
                    });
                }
            }
        }
        if state.selected {
            canvas.outline(cover, theme.accent);
        }

        let line = (caption_px / 3).max(1);
        let name = Rect::new(
            rect.left,
            cover.bottom + 1,
            rect.right,
            cover.bottom + 1 + line,
        );
        let artist = name.offset(0, line);
        let year = artist.offset(0, line);
        let format = TextFormat::left()
            .vcenter()
            .single_line()
            .end_ellipsis()
            .no_prefix();
        if let Some(font) = &bold {
            canvas.with_font(font, |canvas| {
                canvas.draw_text(name, &tile.name, theme.text, format);
            });
        }
        if let Some(font) = &body {
            canvas.with_font(font, |canvas| {
                canvas.draw_text(artist, &tile.artist, theme.text_secondary, format);
            });
        }
        if let (Some(year_value), Some(font)) = (tile.year, &small) {
            canvas.with_font(font, |canvas| {
                canvas.draw_text(year, &year_value.to_string(), theme.text_secondary, format);
            });
        }
    }
}

/// Deterministic cover colour derived from an album name, shown until real
/// artwork is decoded (or permanently when the album has none). Mirrors the
/// egui frontend's low-saturation/mid-value HSV so both look the same.
fn placeholder_color(seed: &str) -> Color {
    let mut value = 0u64;
    for byte in seed.bytes() {
        value = value.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    let hue = (value % 360) as f32 / 360.0;
    let saturation = 0.35 + ((value / 360) % 40) as f32 / 100.0;
    let brightness = 0.35 + ((value / 14_400) % 30) as f32 / 100.0;
    hsv(hue, saturation, brightness)
}

/// Converts HSV (`h` in `0.0..1.0`) to an opaque RGB colour.
fn hsv(h: f32, s: f32, v: f32) -> Color {
    let sector = h * 6.0;
    let index = sector.floor() as i32;
    let f = sector - index as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match index.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::rgb(channel(r), channel(g), channel(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_colour_is_deterministic() {
        assert_eq!(placeholder_color("Album"), placeholder_color("Album"));
        assert_ne!(placeholder_color("Album"), placeholder_color("Other"));
    }

    #[test]
    fn hsv_primaries_are_exact() {
        assert_eq!(hsv(0.0, 1.0, 1.0), Color::rgb(255, 0, 0));
        assert_eq!(hsv(1.0 / 3.0, 1.0, 1.0), Color::rgb(0, 255, 0));
        assert_eq!(hsv(2.0 / 3.0, 1.0, 1.0), Color::rgb(0, 0, 255));
    }
}
