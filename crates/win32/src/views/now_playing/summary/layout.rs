//! Geometry of the now-playing summary (#110): where each part sits, given the
//! widget's client rectangle and DPI. Pure and unit-tested — painting and
//! hit-testing both read this one layout.

use win32ui::Rect;

use super::{ARTWORK_EDGE, GAP, LINE, LINK_GAP, PAD, STAR, TITLE_LINE};

/// The rectangles the summary's parts occupy, in client coordinates.
pub(super) struct SummaryLayout {
    pub(super) artwork: Rect,
    pub(super) star: Rect,
    pub(super) title: Rect,
    pub(super) artist: Rect,
    pub(super) album: Rect,
    pub(super) details: Rect,
    pub(super) path: Rect,
    pub(super) links: Rect,
    /// Gap between two links, in pixels.
    pub(super) link_gap: i32,
    pub(super) module: Option<ModuleRects>,
}

/// The tracker-module block's line rectangles.
pub(super) struct ModuleRects {
    pub(super) separator: Rect,
    pub(super) header: Rect,
    pub(super) summary: Rect,
    pub(super) order_row: Rect,
    pub(super) message: Rect,
    pub(super) details: Rect,
}

impl SummaryLayout {
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
        let module = has_module.then(|| {
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
        });

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

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Rect {
        Rect::new(0, 0, 280, 600)
    }

    #[test]
    fn artwork_is_a_square_within_the_panel() {
        let layout = SummaryLayout::compute(bounds(), 96, false);
        assert_eq!(layout.artwork.width(), layout.artwork.height());
        assert!(layout.artwork.width() <= 280);
        assert_eq!(layout.artwork.left, (280 - layout.artwork.width()) / 2);
    }

    #[test]
    fn lines_stack_without_overlapping() {
        let layout = SummaryLayout::compute(bounds(), 96, true);
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
        assert!(layout.artwork.bottom <= layout.star.top);
        let module = layout.module.expect("module block");
        assert!(layout.links.bottom <= module.separator.top);
        assert!(module.separator.bottom <= module.header.top);
        assert!(module.header.bottom <= module.summary.top);
        assert!(module.summary.bottom <= module.order_row.top);
        assert!(module.order_row.bottom <= module.message.top);
        assert!(module.message.bottom <= module.details.top);
    }

    #[test]
    fn dpi_scales_the_lines() {
        let normal = SummaryLayout::compute(bounds(), 96, false);
        let scaled = SummaryLayout::compute(bounds(), 192, false);
        assert!(scaled.artist.height() > normal.artist.height());
        assert_eq!(scaled.artist.height(), normal.artist.height() * 2);
    }
}
