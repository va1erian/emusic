//! Painting and accessibility helpers for the navigator widget (#109, #476):
//! the row bands, icon glyphs and labels, and the accessibility node walk.

use emusic_ui::library_api::PlaylistInfo;
use win32ui::gdi::{Canvas, Font, TextFormat};
use win32ui::{Rect, Theme};

use super::rows::{self, Row};

/// A node of the accessibility tree, in render order.
pub(super) enum NodeKind<'a> {
    Heading(&'a str),
    View(emusic_ui::state::View),
    Add,
    Playlist(&'a PlaylistInfo),
}

/// Walks the accessibility nodes in render order: section headings, then
/// views, then the playlists heading, its `+` and the playlist rows.
pub(super) fn for_each_node<'a>(
    dpi: u32,
    bounds: Rect,
    playlists: &'a [PlaylistInfo],
    mut f: impl FnMut(NodeKind<'a>, Rect),
) {
    rows::for_each_row(dpi, bounds, playlists, |row, rect| match row {
        Row::Heading { text, add } => {
            f(NodeKind::Heading(text), rect);
            if add {
                f(NodeKind::Add, rows::add_button_rect(dpi, rect));
            }
        }
        Row::View(view) => f(NodeKind::View(view), rect),
        Row::Playlist(info) => f(NodeKind::Playlist(info), rect),
    });
}

/// Paints a row's selection / hover band.
pub(super) fn paint_row(canvas: &Canvas, rect: Rect, theme: &Theme, selected: bool, hot: bool) {
    if selected {
        canvas.fill_rect(rect, theme.selection);
    } else if hot {
        canvas.fill_rect(rect, theme.hover);
    }
}

/// Paints a row's icon glyph in `font`.
pub(super) fn paint_icon(
    canvas: &Canvas,
    rect: Rect,
    theme: &Theme,
    font: Option<&Font>,
    glyph: &str,
    selected: bool,
    scale: f32,
) {
    let Some(font) = font else {
        return;
    };
    let icon_left = (rows::ICON_LEFT * scale).round() as i32;
    let icon_size = (rows::ICON_SIZE * scale).round() as i32;
    let icon_rect = Rect::new(
        rect.left + icon_left,
        rect.top,
        rect.left + icon_left + icon_size,
        rect.bottom,
    );
    let color = if selected {
        theme.accent
    } else {
        theme.text_secondary
    };
    let format = TextFormat::left()
        .center()
        .vcenter()
        .single_line()
        .no_prefix();
    canvas.with_font(font, |canvas| {
        canvas.draw_text(icon_rect, glyph, color, format);
    });
}

/// Paints a row's label in `font`.
pub(super) fn paint_label(
    canvas: &Canvas,
    rect: Rect,
    theme: &Theme,
    font: Option<&Font>,
    text: &str,
    label_left: i32,
) {
    let Some(font) = font else {
        return;
    };
    let text_rect = Rect::new(rect.left + label_left, rect.top, rect.right, rect.bottom);
    let format = TextFormat::left()
        .vcenter()
        .single_line()
        .end_ellipsis()
        .no_prefix();
    canvas.with_font(font, |canvas| {
        canvas.draw_text(text_rect, text, theme.text, format);
    });
}
