//! Painting the now-playing summary (#110) with GDI from semantic theme
//! tokens. Reads the display snapshot and the layout; records the clickable
//! regions into the widget's hit table as it draws them.

use emusic_ui::views::now_playing::ModuleView;
use win32ui::gdi::{Canvas, Font, TextFormat};
use win32ui::{Color, Rect, Theme};

use super::input::Hit;
use super::layout::SummaryLayout;
use super::{Fonts, SummaryWidget};

/// Paints the whole widget: background, artwork and the metadata/module block.
pub(super) fn paint(widget: &SummaryWidget, canvas: &Canvas, bounds: Rect, theme: &Theme) {
    canvas.fill_rect(bounds, theme.background);
    let has_module = widget.data.borrow().module.is_some();
    let layout = SummaryLayout::compute(bounds, widget.dpi.get(), has_module);

    canvas.fill_rect(layout.artwork, theme.input_background);
    let artwork = widget.artwork.borrow();
    match artwork.as_ref() {
        Some(bitmap) => canvas.draw_bitmap(bitmap, layout.artwork),
        None => draw_text(
            canvas,
            widget.fonts.borrow().title.as_ref(),
            layout.artwork,
            "\u{266A}",
            theme.text_secondary,
            TextFormat::left()
                .center()
                .vcenter()
                .single_line()
                .no_prefix(),
        ),
    }
    drop(artwork);

    paint_summary(widget, canvas, &layout, theme);
}

fn paint_summary(widget: &SummaryWidget, canvas: &Canvas, layout: &SummaryLayout, theme: &Theme) {
    let data = widget.data.borrow();
    let fonts = widget.fonts.borrow();
    let mut hits = widget.hits.borrow_mut();
    hits.clear();

    if !data.playing {
        draw_text(
            canvas,
            fonts.body.as_ref(),
            layout.artist,
            "Nothing playing",
            theme.text_secondary,
            TextFormat::left().vcenter().single_line().no_prefix(),
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
            glyph,
            color,
            TextFormat::left().vcenter().single_line().no_prefix(),
        );
        fill_hot(
            canvas,
            layout.star,
            widget.hot.get() == Some(Hit::Star),
            theme,
        );
        hits.push((layout.star, Hit::Star));
    }

    // Title.
    draw_text(
        canvas,
        fonts.title.as_ref(),
        layout.title,
        &data.title,
        theme.text,
        TextFormat::left()
            .vcenter()
            .single_line()
            .end_ellipsis()
            .no_prefix(),
    );

    // Artist link.
    if !data.artist.is_empty() {
        fill_hot(
            canvas,
            layout.artist,
            widget.hot.get() == Some(Hit::Artist),
            theme,
        );
        draw_text(
            canvas,
            fonts.body.as_ref(),
            layout.artist,
            &data.artist,
            theme.accent,
            TextFormat::left()
                .vcenter()
                .single_line()
                .end_ellipsis()
                .no_prefix(),
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
            );
            hits.push((layout.album, Hit::Album));
            theme.accent
        };
        draw_text(
            canvas,
            fonts.body.as_ref(),
            layout.album,
            &data.album_line,
            color,
            TextFormat::left()
                .vcenter()
                .single_line()
                .end_ellipsis()
                .no_prefix(),
        );
    }

    // Technical details line.
    if let Some(details) = &data.details {
        draw_text(
            canvas,
            fonts.small.as_ref(),
            layout.details,
            details,
            theme.text_secondary,
            TextFormat::left()
                .vcenter()
                .single_line()
                .end_ellipsis()
                .no_prefix(),
        );
    }

    // Path link.
    if !data.path.is_empty() {
        fill_hot(
            canvas,
            layout.path,
            widget.hot.get() == Some(Hit::Path),
            theme,
        );
        let text = emusic_ui::views::now_playing::truncate_path(&data.path);
        draw_text(
            canvas,
            fonts.small.as_ref(),
            layout.path,
            &text,
            theme.accent,
            TextFormat::left()
                .vcenter()
                .single_line()
                .end_ellipsis()
                .no_prefix(),
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
            let width = measure_text(canvas, fonts.small.as_ref(), text);
            let rect = Rect::new(x, layout.links.top, x + width, layout.links.bottom);
            fill_hot(canvas, rect, widget.hot.get() == Some(hit), theme);
            draw_text(
                canvas,
                fonts.small.as_ref(),
                rect,
                text,
                theme.accent,
                TextFormat::left().vcenter().single_line().no_prefix(),
            );
            hits.push((rect, hit));
            x += width + layout.link_gap;
        }
    }

    if let Some(module) = &data.module {
        paint_module(canvas, layout, module, theme, &fonts);
    }
}

fn paint_module(
    canvas: &Canvas,
    layout: &SummaryLayout,
    module: &ModuleView,
    theme: &Theme,
    fonts: &Fonts,
) {
    let Some(rects) = &layout.module else {
        return;
    };
    canvas.fill_rect(rects.separator, theme.border);
    draw_text(
        canvas,
        fonts.small.as_ref(),
        rects.header,
        "MODULE",
        theme.text_secondary,
        TextFormat::left().vcenter().single_line().no_prefix(),
    );
    draw_text(
        canvas,
        fonts.body.as_ref(),
        rects.summary,
        &module.summary_text(),
        theme.text,
        TextFormat::left()
            .vcenter()
            .single_line()
            .end_ellipsis()
            .no_prefix(),
    );
    draw_text(
        canvas,
        fonts.body.as_ref(),
        rects.order_row,
        &module.order_row_text(),
        theme.text_secondary,
        TextFormat::left().vcenter().single_line().no_prefix(),
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
        &message,
        theme.text_secondary,
        TextFormat::left()
            .vcenter()
            .single_line()
            .end_ellipsis()
            .no_prefix(),
    );
    draw_text(
        canvas,
        fonts.small.as_ref(),
        rects.details,
        &module.format,
        theme.text_secondary,
        TextFormat::left().vcenter().single_line().no_prefix(),
    );
}

/// Draws `text` with `font` if it exists.
fn draw_text(
    canvas: &Canvas,
    font: Option<&Font>,
    rect: Rect,
    text: &str,
    color: Color,
    format: TextFormat,
) {
    if let Some(font) = font {
        canvas.with_font(font, |canvas| {
            canvas.draw_text(rect, text, color, format);
        });
    }
}

/// Measures `text` with `font`, or zero when the font is missing.
fn measure_text(canvas: &Canvas, font: Option<&Font>, text: &str) -> i32 {
    font.map_or(0, |font| {
        canvas.with_font(font, |c| c.text_size(text).width)
    })
}

/// Fills `rect` with the hover colour when `hot`.
fn fill_hot(canvas: &Canvas, rect: Rect, hot: bool, theme: &Theme) {
    if hot {
        canvas.fill_rect(rect, theme.hover);
    }
}
