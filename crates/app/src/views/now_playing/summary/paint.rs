//! Geometry and painting for the now-playing summary (#110, #247).
//!
//! The [`Layout`] works in device pixels and is shared by painting and hit
//! testing; [`paint`] draws it through the portable `Canvas`, recording each
//! clickable region into the widget's hit table as it draws it.

use emusic_ui::views::now_playing::ModuleView;
use xui::xui_core::Image;
use xui::xui_core::Theme;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::{Canvas, TextStyle};
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::{Dip, dip};

use super::{Hit, SummaryData};
use crate::app::Msg;

/// Artwork box edge, in design units.
const ARTWORK_EDGE: f32 = 200.0;
/// Padding around the summary content.
const PAD: f32 = 8.0;
/// Gap between blocks.
const GAP: f32 = 8.0;
/// A regular text line's height.
const LINE: f32 = 18.0;
/// Height of the star + title row.
const TITLE_LINE: f32 = 22.0;
/// Width of the star hit box.
const STAR: f32 = 20.0;
/// Gap between the star and the title, or between two links.
const LINK_GAP: f32 = 6.0;
/// Title text size.
const TITLE_SIZE: Dip = dip(15.0);
/// Body text size.
const BODY_SIZE: Dip = dip(12.0);
/// Caption text size.
const SMALL_SIZE: Dip = dip(10.0);

/// The rectangles the summary's parts occupy, in client pixels.
pub(super) struct Layout {
    pub(super) artwork: Rect,
    pub(super) star: Rect,
    pub(super) title: Rect,
    pub(super) artist: Rect,
    pub(super) album: Rect,
    pub(super) details: Rect,
    pub(super) path: Rect,
    pub(super) links: Rect,
    pub(super) link_gap: i32,
    pub(super) module: Option<ModuleRects>,
}

/// The tracker-module block's line rectangles.
pub(super) struct ModuleRects {
    separator: Rect,
    header: Rect,
    summary: Rect,
    order_row: Rect,
    message: Rect,
    details: Rect,
}

impl Layout {
    pub(super) fn compute(bounds: Rect, dpi: u32, has_module: bool) -> Self {
        let scale = dpi as f32 / 96.0;
        let px = |value: f32| (value * scale).round() as i32;
        let pad = px(PAD);
        let line = px(LINE);
        let left = bounds.left + pad;
        let right = bounds.right - pad;
        let inner = (bounds.width() - 2 * pad).max(0);
        let edge = inner.min(px(ARTWORK_EDGE)).max(0);
        let artwork_left = bounds.left + (bounds.width() - edge) / 2;

        let mut y = bounds.top + pad;
        let artwork = Rect::new(artwork_left, y, artwork_left + edge, y + edge);
        y = artwork.bottom + px(GAP);

        let star = Rect::new(left, y, left + px(STAR), y + line);
        let title = Rect::new(star.right + px(4.0), y, right, y + px(TITLE_LINE));
        y += px(TITLE_LINE);
        let artist = Rect::new(left, y, right, y + line);
        y += line;
        let album = Rect::new(left, y, right, y + line);
        y += line;
        let details = Rect::new(left, y, right, y + line);
        y += line;
        let path = Rect::new(left, y, right, y + line);
        y += line;
        let links = Rect::new(left, y, right, y + line);
        y += line;
        let module = has_module
            .then(|| {
                y += px(GAP);
                let separator = Rect::new(left, y, right, y + 1);
                y += px(4.0);
                let header = Rect::new(left, y, right, y + line);
                y += line;
                let summary = Rect::new(left, y, right, y + line);
                y += line;
                let order_row = Rect::new(left, y, right, y + line);
                y += line;
                let message = Rect::new(left, y, right, y + line);
                y += line;
                let details = Rect::new(left, y, right, y + line);
                ModuleRects {
                    separator,
                    header,
                    summary,
                    order_row,
                    message,
                    details,
                }
            })
            // Only show the block when the whole of it fits; a clipped
            // "MODULE" heading over the queue reads as a glitch.
            .filter(|module| module.details.bottom <= bounds.bottom);

        Self {
            artwork,
            star,
            title,
            artist,
            album,
            details,
            path,
            links,
            link_gap: px(LINK_GAP),
            module,
        }
    }
}

/// Paints the whole summary: background, artwork and the metadata/module block.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    canvas: &mut dyn Canvas,
    ui: &Ui<Msg>,
    theme: &Theme,
    data: &SummaryData,
    artwork: Option<&Image>,
    hits: &mut Vec<(Rect, Hit)>,
    hot: Option<Hit>,
) {
    let bounds = canvas.bounds();
    canvas.clear(theme.background);
    let layout = Layout::compute(bounds, canvas.dpi(), data.module.is_some());

    canvas.fill_rect(layout.artwork, theme.surface);
    match artwork {
        Some(image) => canvas.draw_image(image, layout.artwork),
        None => draw_text(
            canvas,
            layout.artwork,
            "\u{266A}",
            &TextStyle::new(theme.text_secondary, TITLE_SIZE)
                .centered()
                .middle(),
        ),
    }

    hits.clear();
    if !data.playing {
        draw_text(
            canvas,
            layout.artist,
            "Nothing playing",
            &TextStyle::new(theme.text_secondary, BODY_SIZE).middle(),
        );
        return;
    }

    if data.has_track {
        let glyph = if data.starred { "\u{2605}" } else { "\u{2606}" };
        let color = if data.starred {
            theme.accent
        } else {
            theme.text_secondary
        };
        draw_text(
            canvas,
            layout.star,
            glyph,
            &TextStyle::new(color, BODY_SIZE).middle(),
        );
        fill_hot(canvas, layout.star, hot == Some(Hit::Star), theme);
        hits.push((layout.star, Hit::Star));
    }

    draw_text(
        canvas,
        layout.title,
        &data.title,
        &TextStyle::new(theme.text, TITLE_SIZE).bold().middle(),
    );

    if !data.artist.is_empty() {
        fill_hot(canvas, layout.artist, hot == Some(Hit::Artist), theme);
        draw_text(
            canvas,
            layout.artist,
            &data.artist,
            &TextStyle::new(theme.accent, BODY_SIZE).middle(),
        );
        hits.push((layout.artist, Hit::Artist));
    }

    if !data.album_line.is_empty() {
        let color = if data.album.is_empty() {
            theme.text_secondary
        } else {
            fill_hot(canvas, layout.album, hot == Some(Hit::Album), theme);
            hits.push((layout.album, Hit::Album));
            theme.accent
        };
        draw_text(
            canvas,
            layout.album,
            &data.album_line,
            &TextStyle::new(color, BODY_SIZE).middle(),
        );
    }

    if let Some(details) = &data.details {
        draw_text(
            canvas,
            layout.details,
            details,
            &TextStyle::new(theme.text_secondary, SMALL_SIZE).middle(),
        );
    }

    if !data.path.is_empty() {
        let text = emusic_ui::views::now_playing::truncate_path(&data.path);
        draw_text(
            canvas,
            layout.path,
            &text,
            &TextStyle::new(theme.accent, SMALL_SIZE).middle(),
        );
    }

    if data.has_track {
        let mut x = layout.links.left;
        for (text, hit) in [
            ("Properties\u{2026}", Hit::Properties),
            ("Edit tags\u{2026}", Hit::EditTags),
        ] {
            let width = ui
                .measure_text(
                    text,
                    &TextStyle::new(theme.accent, SMALL_SIZE),
                    canvas.dpi(),
                )
                .width;
            let rect = Rect::new(x, layout.links.top, x + width, layout.links.bottom);
            fill_hot(canvas, rect, hot == Some(hit), theme);
            draw_text(
                canvas,
                rect,
                text,
                &TextStyle::new(theme.accent, SMALL_SIZE).middle(),
            );
            hits.push((rect, hit));
            x += width + layout.link_gap;
        }
    }

    if let Some(module) = &data.module {
        paint_module(canvas, &layout, module, theme);
    }
}

/// Paints the tracker-module block.
fn paint_module(canvas: &mut dyn Canvas, layout: &Layout, module: &ModuleView, theme: &Theme) {
    let Some(rects) = &layout.module else {
        return;
    };
    canvas.fill_rect(rects.separator, theme.border);
    draw_text(
        canvas,
        rects.header,
        "MODULE",
        &TextStyle::new(theme.text_secondary, SMALL_SIZE).middle(),
    );
    draw_text(
        canvas,
        rects.summary,
        &module.summary_text(),
        &TextStyle::new(theme.text, BODY_SIZE).middle(),
    );
    draw_text(
        canvas,
        rects.order_row,
        &module.order_row_text(),
        &TextStyle::new(theme.text_secondary, BODY_SIZE).middle(),
    );
    let message = if module.message.is_empty() {
        module.details_label()
    } else {
        module.message.clone()
    };
    draw_text(
        canvas,
        rects.message,
        &message,
        &TextStyle::new(theme.text_secondary, SMALL_SIZE).middle(),
    );
    draw_text(
        canvas,
        rects.details,
        &module.format,
        &TextStyle::new(theme.text_secondary, SMALL_SIZE).middle(),
    );
}

/// Draws single-line text clipped to `rect`.
fn draw_text(canvas: &mut dyn Canvas, rect: Rect, text: &str, style: &TextStyle) {
    if rect.width() <= 0 || rect.height() <= 0 || text.is_empty() {
        return;
    }
    canvas.push_clip(rect);
    canvas.draw_text(text, rect, style);
    canvas.pop_clip();
}

/// Fills `rect` with the hover colour when `hot`.
fn fill_hot(canvas: &mut dyn Canvas, rect: Rect, hot: bool, theme: &Theme) {
    if hot {
        canvas.fill_rect(rect, theme.hover);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_lines_stack_without_overlapping() {
        let layout = Layout::compute(Rect::new(0, 0, 600, 700), 96, true);
        assert_eq!(layout.artwork.width(), layout.artwork.height());
        let rows = [
            layout.star,
            layout.artist,
            layout.album,
            layout.details,
            layout.path,
            layout.links,
        ];
        for pair in rows.windows(2) {
            assert!(pair[0].bottom <= pair[1].top, "rows must not overlap");
        }
        let module = layout.module.expect("module block");
        assert!(layout.links.bottom <= module.separator.top);
        assert!(module.order_row.bottom <= module.message.top);
    }
}
