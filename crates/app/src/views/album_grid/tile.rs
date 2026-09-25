//! The album grid's tiles (#113): the model item, its [`GridModel`] wrapper and
//! the `content` painter that draws a cover, its caption and the selection.
//!
//! The painter receives a [`D2dCanvas`] and the tile rectangle from the shared
//! [`GridView`](win32ui::GridView); it uploads the cached cover into Direct2D's
//! bitmap cache once per decode and draws it scaled, then the album/artist/year
//! caption. Cover colour and caption formatting come from the shared
//! `album_grid` model.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use emusic_ui::state::Metrics;
use emusic_ui::views::album_grid::models::AlbumKey;
use win32ui::d2d::{D2dCanvas, Font, FontSpec, ImageId, Interpolation, RectF, Stroke, TextSystem};
use win32ui::prelude::*;

use crate::d2d_text::{self, Align, LineStyle};

use super::thumbs::ThumbState;

/// Caption height reserved under the cover, in design units.
pub(super) const CAPTION_DIP: f32 = 42.0;

/// Font roles for [`TileFonts::draw`]'s layout cache, one per caption font.
const ROLE_BOLD: u8 = 0;
const ROLE_BODY: u8 = 1;
const ROLE_SMALL: u8 = 2;
const ROLE_GLYPH: u8 = 3;

/// One album tile: the identity used for messages plus the caption fields and
/// the source path its cover is read from.
pub(super) struct AlbumTile {
    pub(super) key: AlbumKey,
    pub(super) name: String,
    pub(super) artist: String,
    pub(super) year: Option<u32>,
    pub(super) art_path: String,
    /// The tile's cover uploaded to the Direct2D surface, kept so a decode is
    /// uploaded once rather than every frame.
    cover: Cell<Option<CoverImage>>,
}

impl AlbumTile {
    /// A tile with no cover uploaded yet.
    pub(super) fn new(
        key: AlbumKey,
        name: String,
        artist: String,
        year: Option<u32>,
        art_path: String,
    ) -> Self {
        Self {
            key,
            name,
            artist,
            year,
            art_path,
            cover: Cell::new(None),
        }
    }
}

/// An album's cover uploaded to the Direct2D surface: the RGBA buffer's
/// address identifies the decode, so a re-decoded cover re-uploads.
#[derive(Clone, Copy)]
struct CoverImage {
    source: usize,
    id: ImageId,
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

/// The DirectWrite fonts the tile caption is drawn with, keyed by the metrics
/// they were built from so an appearance change rebuilds them once.
struct TileFonts {
    metrics: Metrics,
    body: Option<Font>,
    bold: Option<Font>,
    small: Option<Font>,
    glyph: Option<Font>,
    /// Laid-out single-line captions, so a repaint that only changes the
    /// selection (or scrolls) reuses them.
    captions: d2d_text::LineCache,
}

impl TileFonts {
    fn build(system: Option<&TextSystem>, metrics: Metrics) -> Self {
        // `FontSpec` takes an em size in DIPs, which is exactly what `Metrics`
        // carries.
        let font = |family: &str, dip: f32, weight: u16| {
            system.and_then(|system| system.font(&FontSpec::new(family, dip).weight(weight)).ok())
        };
        let scale = metrics.body / crate::appearance::BASE_BODY_DIP;
        // 22 pt was the placeholder glyph's size; keep it at the default scale.
        let glyph_dip = 22.0 * 96.0 / 72.0 * scale;
        Self {
            metrics,
            body: font(crate::appearance::UI_FAMILY, metrics.body, 400),
            bold: font(crate::appearance::UI_FAMILY, metrics.body, 700),
            small: font(crate::appearance::UI_FAMILY, metrics.small, 400),
            glyph: font("Segoe UI Symbol", glyph_dip, 400),
            captions: d2d_text::LineCache::default(),
        }
    }

    /// Draws one caption line on `rect`, reusing its cached layout.
    fn draw(
        &self,
        canvas: &mut D2dCanvas<'_>,
        role: u8,
        font: Option<&Font>,
        rect: RectF,
        text: &str,
        style: LineStyle,
    ) {
        if let Some(font) = font {
            self.captions.draw(canvas, role, font, rect, text, style);
        }
    }
}

/// Builds the `content` painter, capturing the shared thumbnail state, the
/// current theme and the fonts it draws with. The fonts follow the live
/// appearance metrics, rebuilt when they change (#309).
pub(super) fn content(
    thumbs: Rc<RefCell<ThumbState>>,
    theme: Rc<Cell<Theme>>,
) -> impl Fn(&AlbumTile, &mut D2dCanvas<'_>, RectF, TileState) + 'static {
    let system = TextSystem::new().ok();
    let fonts = RefCell::new(TileFonts::build(
        system.as_ref(),
        crate::appearance::metrics(),
    ));

    move |tile, canvas, rect, state| {
        let theme = theme.get();
        {
            let metrics = crate::appearance::metrics();
            let mut fonts = fonts.borrow_mut();
            if fonts.metrics != metrics {
                *fonts = TileFonts::build(system.as_ref(), metrics);
            }
        }
        let fonts = fonts.borrow();
        let (body, bold, small, glyph) = (&fonts.body, &fonts.bold, &fonts.small, &fonts.glyph);
        let edge = (rect.width() - CAPTION_DIP).max(1.0);
        let cover = RectF::new(rect.left, rect.top, rect.left + edge, rect.top + edge);

        match thumbs.borrow_mut().cover(&tile.art_path) {
            Some(image) => {
                let source = Rc::as_ptr(&image) as usize;
                let id = match tile.cover.get() {
                    Some(cached) if cached.source == source => cached.id,
                    _ => {
                        if let Some(stale) = tile.cover.get() {
                            canvas.forget_image(stale.id);
                        }
                        let id = canvas.image(&image);
                        tile.cover.set(Some(CoverImage { source, id }));
                        id
                    }
                };
                canvas.draw_image(id, cover, None, 1.0, Interpolation::Linear);
            }
            None => {
                tile.cover.set(None);
                canvas.fill_rect(cover, placeholder_color(&tile.name));
                fonts.draw(
                    canvas,
                    ROLE_GLYPH,
                    glyph.as_ref(),
                    cover,
                    "\u{266A}",
                    LineStyle {
                        color: Color::rgb(255, 255, 255),
                        align: Align::Center,
                    },
                );
            }
        }
        if state.selected {
            canvas.stroke_rect(cover, theme.accent, Stroke::solid(1.0));
        }

        let line = CAPTION_DIP / 3.0;
        let name = RectF::new(
            rect.left,
            cover.bottom + 1.0,
            rect.right,
            cover.bottom + 1.0 + line,
        );
        let artist = RectF::new(name.left, name.top + line, name.right, name.bottom + line);
        let year = RectF::new(
            artist.left,
            artist.top + line,
            artist.right,
            artist.bottom + line,
        );
        fonts.draw(
            canvas,
            ROLE_BOLD,
            bold.as_ref(),
            name,
            &tile.name,
            LineStyle::left(theme.text),
        );
        fonts.draw(
            canvas,
            ROLE_BODY,
            body.as_ref(),
            artist,
            &tile.artist,
            LineStyle::left(theme.text_secondary),
        );
        if let Some(year_value) = tile.year {
            fonts.draw(
                canvas,
                ROLE_SMALL,
                small.as_ref(),
                year,
                &year_value.to_string(),
                LineStyle::left(theme.text_secondary),
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
