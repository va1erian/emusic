//! Pure text and geometry for the iconic taskbar thumbnail (#322).
//!
//! Kept separate from the GDI rendering so the layout and the time formatting
//! can be unit-tested without a window. All coordinates are device pixels
//! relative to the thumbnail's top-left corner.

/// A cover image borrowed for one render: RGBA8, row-major, top-down.
pub struct Cover<'a> {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes of RGBA.
    pub rgba: &'a [u8],
}

/// Everything the taskbar thumbnail shows for the current track.
pub struct Panel<'a> {
    /// Track title, or the app name when nothing is playing.
    pub title: &'a str,
    /// Track artist; may be empty.
    pub artist: &'a str,
    /// Album name; may be empty.
    pub album: &'a str,
    /// Playback position, in whole seconds.
    pub elapsed_secs: u64,
    /// Track length, in whole seconds (`0` when unknown).
    pub total_secs: u64,
    /// Decoded cover art, when available.
    pub cover: Option<Cover<'a>>,
}

/// The thumbnail size DWM asked for, in device pixels.
///
/// DWM passes its maximum in `WM_DWMSENDICONICTHUMBNAIL`; the panel is
/// rendered at exactly this size so DWM never has to scale it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThumbnailSize {
    /// Maximum width in pixels.
    pub width: u32,
    /// Maximum height in pixels.
    pub height: u32,
}

/// A pixel rectangle relative to the thumbnail's top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    /// The empty rectangle at the origin.
    pub const EMPTY: Rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    /// Width in pixels; never negative.
    #[must_use]
    pub fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }

    /// Height in pixels; never negative.
    #[must_use]
    pub fn height(self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    /// Whether the rectangle has no area.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width() == 0 || self.height() == 0
    }
}

/// The rectangles one panel draws into, plus the font sizes to draw with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelLayout {
    /// Cover art; empty when the thumbnail is too small for one.
    pub cover: Rect,
    pub title: Rect,
    pub artist: Rect,
    pub album: Rect,
    /// The `elapsed / total` line.
    pub time: Rect,
    /// Title font height in pixels.
    pub title_px: i32,
    /// Artist / album / time font height in pixels.
    pub body_px: i32,
}

/// The narrowest text column worth drawing into.
const MIN_TEXT_WIDTH: i32 = 28;

/// Lays the panel out for `size`: cover art on the left, the text stack on
/// the right. A thumbnail too small for one of them leaves that part empty.
///
/// The fonts are sized from the card, not from the line rectangles: a line
/// rectangle a third of the card tall would give a title font far too large to
/// fit more than a few characters. `title_px` / `body_px` are kept apart so
/// the renderer draws exactly the size the layout reserved room for.
#[must_use]
pub fn layout(size: ThumbnailSize) -> PanelLayout {
    let width = size.width as i32;
    let height = size.height as i32;
    let empty = PanelLayout {
        cover: Rect::EMPTY,
        title: Rect::EMPTY,
        artist: Rect::EMPTY,
        album: Rect::EMPTY,
        time: Rect::EMPTY,
        title_px: 0,
        body_px: 0,
    };
    if width <= 0 || height <= 0 {
        return empty;
    }

    // Padding (and the text's leading) scales with the card but stays legible.
    let pad = (height / 20).clamp(3, 12);
    let inner_height = (height - 2 * pad).max(0);
    // The cover keeps its square aspect and never takes more than half the
    // width, so the text column has room.
    let cover_side = inner_height.min(((width - 3 * pad) / 2).max(0));
    let cover = Rect {
        left: pad,
        top: pad,
        right: pad + cover_side,
        bottom: pad + cover_side,
    };

    let text_left = cover.right + pad;
    let text_right = width - pad;
    let text_width = text_right - text_left;
    if text_width < MIN_TEXT_WIDTH || inner_height < 4 * pad {
        return PanelLayout { cover, ..empty };
    }

    // Modest, card-proportional sizes: roughly a sixth of the height for the
    // title and a tenth for the body, so a normal title reads in full.
    let title_px = (height * 12 / 100).clamp(9, 22);
    let body_px = (height * 9 / 100).clamp(7, 16);
    // A line rectangle gives the font a little leading to sit in.
    let title_height = title_px * 3 / 2;
    let body_height = body_px * 3 / 2;

    let line = |top: i32, line_height: i32| Rect {
        left: text_left,
        top,
        right: text_right,
        bottom: top + line_height,
    };
    let mut top = pad;
    let title = line(top, title_height);
    top += title_height;
    let artist = line(top, body_height);
    top += body_height;
    let album = line(top, body_height);
    // The time line is bottom-aligned, leaving any slack between album and it.
    let time = line(cover.bottom - body_height, body_height);

    PanelLayout {
        cover,
        title,
        artist,
        album,
        time,
        title_px,
        body_px,
    }
}

/// Formats `secs` as `m:ss`, or `h:mm:ss` once it reaches an hour.
#[must_use]
pub fn format_time(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// The `elapsed / total` line, or just the elapsed time when `total` is
/// unknown (`0`).
#[must_use]
pub fn time_text(elapsed_secs: u64, total_secs: u64) -> String {
    if total_secs == 0 {
        format_time(elapsed_secs)
    } else {
        format!(
            "{} / {}",
            format_time(elapsed_secs),
            format_time(total_secs)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: u32, height: u32) -> ThumbnailSize {
        ThumbnailSize { width, height }
    }

    #[test]
    fn format_time_switches_to_hours_at_an_hour() {
        assert_eq!(format_time(0), "0:00");
        assert_eq!(format_time(5), "0:05");
        assert_eq!(format_time(65), "1:05");
        assert_eq!(format_time(3599), "59:59");
        assert_eq!(format_time(3600), "1:00:00");
        assert_eq!(format_time(7325), "2:02:05");
    }

    #[test]
    fn time_text_drops_the_total_when_unknown() {
        assert_eq!(time_text(65, 0), "1:05");
        assert_eq!(time_text(65, 200), "1:05 / 3:20");
    }

    #[test]
    fn layout_keeps_the_cover_square_and_text_stacked() {
        let layout = layout(size(300, 150));
        assert_eq!(
            layout.cover.width(),
            layout.cover.height(),
            "cover stays square"
        );
        assert!(
            layout.cover.right <= 150,
            "cover takes at most half the width"
        );
        assert!(
            layout.title.left > layout.cover.right,
            "text is to the right"
        );
        assert!(layout.title.top < layout.artist.top, "title above artist");
        assert!(layout.artist.top < layout.album.top, "artist above album");
        assert!(layout.time.bottom <= 150, "time stays on the card");
        assert!(!layout.title.is_empty());
        assert!(!layout.time.is_empty());
    }

    #[test]
    fn layout_leaves_text_empty_when_the_card_is_too_narrow() {
        // A tiny thumbnail has no room for text; a wide one places it to the
        // right of the cover.
        let tiny = layout(size(24, 24));
        assert!(tiny.title.is_empty(), "no room for text");
        let narrow = layout(size(60, 200));
        assert!(narrow.title.is_empty(), "the text column is too narrow");
        assert!(!narrow.cover.is_empty(), "the cover still fits");
        let wide = layout(size(320, 200));
        assert!(wide.title.left >= wide.cover.right);
    }

    #[test]
    fn font_sizes_stay_small_enough_for_the_title_to_read() {
        // DWM asks for roughly 250x137 on a normal taskbar; the old
        // layout-sized fonts came out around 29px and ellipsised the title to
        // a few characters.
        let layout = layout(size(250, 137));
        assert_eq!(layout.title_px, 16);
        assert_eq!(layout.body_px, 12);
        assert!(
            layout.title_px <= layout.body_px * 2,
            "the title should not dwarf the body text"
        );
        assert!(
            layout.title_px < layout.title.height(),
            "the line rectangle leaves leading for the font"
        );
        assert!(layout.time.bottom <= 137, "the time line stays on the card");
    }

    #[test]
    fn layout_of_a_degenerate_size_is_empty() {
        let layout = layout(size(0, 0));
        assert!(layout.cover.is_empty());
        assert!(layout.title.is_empty());
    }

    #[test]
    fn rect_width_and_height_never_go_negative() {
        let inverted = Rect {
            left: 10,
            top: 10,
            right: 5,
            bottom: 5,
        };
        assert_eq!(inverted.width(), 0);
        assert_eq!(inverted.height(), 0);
    }
}
