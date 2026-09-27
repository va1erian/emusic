//! The album grid's tiles (#113), ported to the portable layer: the
//! [`GridModel`] wrapper and the painter that draws a cover, its caption and
//! the selection.
//!
//! The painter receives a portable [`Canvas`] and the tile rectangle from the
//! shared [`GridView`](xui::xui_core::widget::GridView); it draws the cached
//! cover with [`Canvas::draw_image`] and the album/artist/year caption with
//! [`Canvas::draw_text`]. Cover colour and caption formatting come from the
//! shared `album_grid` model.

use std::cell::RefCell;
use std::rc::Rc;

use emusic_ui::views::album_grid::models::AlbumKey;
use xui::xui_core::backend::{Canvas, TextStyle};
use xui::xui_core::units::dip;
use xui::xui_core::widget::{GridModel, Tile, TilePaint, TileSize};
use xui::xui_core::{Color, Dip, Rect};

use super::thumbs::ThumbState;

/// Caption height reserved under the cover, in design units.
pub(super) const CAPTION_DIP: f32 = 42.0;
/// Gap between tiles, in design units.
pub(super) const GAP_DIP: f32 = 12.0;

/// The tile size for a cover `edge`, reserving the caption strip below it.
pub(super) fn tile_size(edge: f32) -> TileSize {
    TileSize::new(dip(edge), dip(edge + CAPTION_DIP)).gap(dip(GAP_DIP))
}

/// Caption font sizes, in design units.
const NAME_SIZE: f32 = 12.0;
const ARTIST_SIZE: f32 = 11.0;
const YEAR_SIZE: f32 = 10.0;
/// The placeholder glyph's size, in design units.
const GLYPH_SIZE: f32 = 28.0;

/// One album tile: the identity used for messages plus the caption fields and
/// the source path its cover is read from.
pub(super) struct AlbumCell {
    pub(super) key: AlbumKey,
    pub(super) name: String,
    pub(super) artist: String,
    pub(super) year: Option<u32>,
    pub(super) art_path: String,
}

/// The grid's owner-data model: the albums in display order.
pub(super) struct TileModel {
    pub(super) cells: Rc<Vec<AlbumCell>>,
}

impl GridModel for TileModel {
    fn len(&self) -> usize {
        self.cells.len()
    }

    fn tile(&self, index: usize) -> Option<Tile<'_>> {
        Some(Tile::new(&self.cells.get(index)?.name))
    }
}

/// Builds the tile painter, which reads the current cells by index and the
/// shared cover cache. The cells handle is replaced on every rebuild, so the
/// painter always sees the album list in sync with the model.
pub(super) fn content(
    cells: Rc<RefCell<Rc<Vec<AlbumCell>>>>,
    thumbs: Rc<RefCell<ThumbState>>,
) -> impl Fn(&mut dyn Canvas, &TilePaint<'_>) + 'static {
    move |canvas, paint| {
        let cells = cells.borrow();
        let Some(cell) = cells.get(paint.index) else {
            return;
        };
        let rect = paint.rect;
        let cover = Rect::new(rect.left, rect.top, rect.right, rect.top + rect.width());

        match thumbs.borrow_mut().cover(&cell.art_path) {
            Some(image) => canvas.draw_image(&image, cover),
            None => {
                canvas.fill_rect(cover, placeholder_color(&cell.name));
                canvas.draw_text(
                    "\u{266A}",
                    cover,
                    &TextStyle::new(Color::rgb(255, 255, 255), Dip(GLYPH_SIZE))
                        .centered()
                        .middle(),
                );
            }
        }
        if paint.selected {
            canvas.stroke_rect(cover, paint.theme.accent, 2.0);
        }

        let line = dip(CAPTION_DIP / 3.0).to_px(paint.dpi).value();
        let name = Rect::new(
            rect.left,
            cover.bottom + 1,
            rect.right,
            cover.bottom + 1 + line,
        );
        let artist = Rect::new(name.left, name.bottom, name.right, name.bottom + line);
        let year = Rect::new(
            artist.left,
            artist.bottom,
            artist.right,
            artist.bottom + line,
        );
        canvas.draw_text(
            &cell.name,
            name,
            &TextStyle::new(paint.theme.text, Dip(NAME_SIZE)).bold(),
        );
        canvas.draw_text(
            &cell.artist,
            artist,
            &TextStyle::new(paint.theme.text_secondary, Dip(ARTIST_SIZE)),
        );
        if let Some(value) = cell.year {
            canvas.draw_text(
                &value.to_string(),
                year,
                &TextStyle::new(paint.theme.text_secondary, Dip(YEAR_SIZE)),
            );
        }
    }
}

/// Deterministic cover colour derived from an album name, shown until real
/// artwork is decoded (or permanently when the album has none), using a
/// low-saturation/mid-value HSV derived from the album name.
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
