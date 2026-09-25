//! Painting the now-playing summary (#110) with Direct2D from semantic theme
//! tokens. Reads the display snapshot and the layout; records the clickable
//! regions into the widget's hit table as it draws them.
//!
//! The shared [`SummaryLayout`] works in device pixels (and hit-testing reads
//! it unchanged); the Direct2D canvas draws in device-independent pixels, so
//! every rectangle is converted with [`rect_to_dip`] first. Text is drawn
//! single-line, vertically centred and elided by [`crate::d2d_text`], matching
//! the GDI path's `DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS`.

use win32ui::d2d::{D2dCanvas, Font, Interpolation, RectF};
use win32ui::{Color, Rect, Theme};

use crate::d2d_text::{self, Align};

use super::input::Hit;
use super::layout::SummaryLayout;
use super::{Fonts, SummaryWidget};

/// Paints the whole widget: background, artwork and the metadata/module block.
pub(super) fn paint(
    widget: &SummaryWidget,
    canvas: &mut D2dCanvas<'_>,
    bounds: RectF,
    theme: &Theme,
) {
    if let Some(id) = widget.forget.take() {
        canvas.forget_image(id);
    }
    let scale = canvas.scale();
    let dpi = (scale * 96.0).round().max(1.0) as u32;
    let has_module = widget.data.borrow().module.is_some();
    let layout = SummaryLayout::compute(d2d_text::rect_to_px(bounds, scale), dpi, has_module);

    let artwork_rect = d2d_text::rect_to_dip(layout.artwork, scale);
    canvas.fill_rect(artwork_rect, theme.input_background);
    let artwork = widget.artwork.borrow();
    match artwork.as_ref() {
        Some(art) => {
            let id = match art.id.get() {
                Some(id) => id,
                None => {
                    let id = canvas.image(&art.image);
                    art.id.set(Some(id));
                    id
                }
            };
            canvas.draw_image(id, artwork_rect, None, 1.0, Interpolation::Linear);
        }
        None => {
            let fonts = widget.fonts.borrow();
            if let Some(font) = &fonts.title {
                d2d_text::draw_line(
                    canvas,
                    font,
                    artwork_rect,
                    "\u{266A}",
                    theme.text_secondary,
                    Align::Center,
                );
            }
        }
    }
    drop(artwork);

    paint_summary(widget, canvas, &layout, scale, theme);
}

fn paint_summary(
    widget: &SummaryWidget,
    canvas: &mut D2dCanvas<'_>,
    layout: &SummaryLayout,
    scale: f32,
    theme: &Theme,
) {
    let data = widget.data.borrow();
    let fonts = widget.fonts.borrow();
    let mut hits = widget.hits.borrow_mut();
    hits.clear();

    if !data.playing {
        draw_text(
            canvas,
            fonts.body.as_ref(),
            layout.artist,
            scale,
            "Nothing playing",
            theme.text_secondary,
        );
        return;
    }

    // Star.
    if data.has_track {
        let glyph = if data.starred { "\u{2605}" } else { "\u{2606}" };
        let color = if data.starred {
            theme.accent
        } else {
            theme.text_secondary
        };
        draw_text(
            canvas,
            fonts.body.as_ref(),
            layout.star,
            scale,
            glyph,
            color,
        );
        fill_hot(
            canvas,
            layout.star,
            widget.hot.get() == Some(Hit::Star),
            theme,
            scale,
        );
        hits.push((layout.star, Hit::Star));
    }

    // Title.
    draw_text(
        canvas,
        fonts.title.as_ref(),
        layout.title,
        scale,
        &data.title,
        theme.text,
    );

    // Artist link.
    if !data.artist.is_empty() {
        fill_hot(
            canvas,
            layout.artist,
            widget.hot.get() == Some(Hit::Artist),
            theme,
            scale,
        );
        draw_text(
            canvas,
            fonts.body.as_ref(),
            layout.artist,
            scale,
            &data.artist,
            theme.accent,
        );
        hits.push((layout.artist, Hit::Artist));
    }

    // Album line link.
    if !data.album_line.is_empty() {
        let color = if data.album.is_empty() {
            theme.text_secondary
        } else {
            fill_hot(
                canvas,
                layout.album,
                widget.hot.get() == Some(Hit::Album),
                theme,
                scale,
            );
            hits.push((layout.album, Hit::Album));
            theme.accent
        };
        draw_text(
            canvas,
            fonts.body.as_ref(),
            layout.album,
            scale,
            &data.album_line,
            color,
        );
    }

    // Technical details line.
    if let Some(details) = &data.details {
        draw_text(
            canvas,
            fonts.small.as_ref(),
            layout.details,
            scale,
            details,
            theme.text_secondary,
        );
    }

    // Path link.
    if !data.path.is_empty() {
        fill_hot(
            canvas,
            layout.path,
            widget.hot.get() == Some(Hit::Path),
            theme,
            scale,
        );
        let text = emusic_ui::views::now_playing::truncate_path(&data.path);
        draw_text(
            canvas,
            fonts.small.as_ref(),
            layout.path,
            scale,
            &text,
            theme.accent,
        );
        hits.push((layout.path, Hit::Path));
    }

    // Properties / Edit tags links.
    if data.has_track {
        let mut x = layout.links.left;
        for (text, hit) in [
            ("Properties\u{2026}", Hit::Properties),
            ("Edit tags\u{2026}", Hit::EditTags),
        ] {
            let width = measure_text(fonts.small.as_ref(), text, scale);
            let rect = Rect::new(x, layout.links.top, x + width, layout.links.bottom);
            fill_hot(canvas, rect, widget.hot.get() == Some(hit), theme, scale);
            draw_text(
                canvas,
                fonts.small.as_ref(),
                rect,
                scale,
                text,
                theme.accent,
            );
            hits.push((rect, hit));
            x += width + layout.link_gap;
        }
    }

    if let Some(module) = &data.module {
        paint_module(canvas, layout, module, theme, &fonts, scale);
    }
}

fn paint_module(
    canvas: &mut D2dCanvas<'_>,
    layout: &SummaryLayout,
    module: &emusic_ui::views::now_playing::ModuleView,
    theme: &Theme,
    fonts: &Fonts,
    scale: f32,
) {
    let Some(rects) = &layout.module else {
        return;
    };
    canvas.fill_rect(d2d_text::rect_to_dip(rects.separator, scale), theme.border);
    draw_text(
        canvas,
        fonts.small.as_ref(),
        rects.header,
        scale,
        "MODULE",
        theme.text_secondary,
    );
    draw_text(
        canvas,
        fonts.body.as_ref(),
        rects.summary,
        scale,
        &module.summary_text(),
        theme.text,
    );
    draw_text(
        canvas,
        fonts.body.as_ref(),
        rects.order_row,
        scale,
        &module.order_row_text(),
        theme.text_secondary,
    );
    let message = if module.message.is_empty() {
        module.details_label()
    } else {
        module.message.clone()
    };
    draw_text(
        canvas,
        fonts.small.as_ref(),
        rects.message,
        scale,
        &message,
        theme.text_secondary,
    );
    draw_text(
        canvas,
        fonts.small.as_ref(),
        rects.details,
        scale,
        &module.format,
        theme.text_secondary,
    );
}

/// Draws `text` on one line with `font`, vertically centred and elided.
fn draw_text(
    canvas: &mut D2dCanvas<'_>,
    font: Option<&Font>,
    rect: Rect,
    scale: f32,
    text: &str,
    color: Color,
) {
    if let Some(font) = font {
        d2d_text::draw_line(
            canvas,
            font,
            d2d_text::rect_to_dip(rect, scale),
            text,
            color,
            Align::Left,
        );
    }
}

/// Measures `text` with `font` in device pixels, or zero when the font is
/// missing.
fn measure_text(font: Option<&Font>, text: &str, scale: f32) -> i32 {
    font.map_or(0, |font| (font.width(text) * scale).round() as i32)
}

/// Fills `rect` with the hover colour when `hot`.
fn fill_hot(canvas: &mut D2dCanvas<'_>, rect: Rect, hot: bool, theme: &Theme, scale: f32) {
    if hot {
        canvas.fill_rect(d2d_text::rect_to_dip(rect, scale), theme.hover);
    }
}
