//! The navigator's row layout (#109, #476): the fixed view sections followed
//! by the dynamic PLAYLISTS section. Kept separate from the widget so the
//! geometry, painting and hit-testing all walk exactly the same rows.

use emusic_ui::library_api::PlaylistInfo;
use emusic_ui::panels::navigator::{PLAYLISTS_HEADING, SECTIONS};
use emusic_ui::state::View;
use win32ui::Rect;

/// Row height, in device-independent pixels.
pub(super) const ROW_HEIGHT: f32 = 26.0;
/// Section-heading row height.
pub(super) const HEADING_HEIGHT: f32 = 24.0;
/// Gap after each section.
pub(super) const SECTION_GAP: f32 = 8.0;
/// Top padding before the first heading.
pub(super) const TOP_PAD: f32 = 4.0;
/// Left inset of an icon.
pub(super) const ICON_LEFT: f32 = 10.0;
/// Icon box size.
pub(super) const ICON_SIZE: f32 = 16.0;
/// Left inset of a row's label (after the icon).
pub(super) const LABEL_LEFT: f32 = 34.0;
/// Left inset of a section heading.
pub(super) const HEADING_LEFT: f32 = 12.0;
/// Side of the `+` button on the playlists heading.
const ADD_BUTTON_SIZE: f32 = 18.0;
/// Right margin of the `+` button from the navigator's edge.
const ADD_RIGHT_MARGIN: f32 = 8.0;

/// One navigator row: a section heading (the playlists heading carries a `+`
/// button), a fixed view, or a user playlist.
pub(super) enum Row<'a> {
    Heading {
        text: &'a str,
        /// Whether the row draws and hit-tests a trailing `+`.
        add: bool,
    },
    View(View),
    Playlist(&'a PlaylistInfo),
}

/// Walks the rows for `bounds`, calling `f` with each row and its client
/// rectangle: the fixed [`SECTIONS`], then [`PLAYLISTS_HEADING`] and one row
/// per playlist.
pub(super) fn for_each_row<'a>(
    dpi: u32,
    bounds: Rect,
    playlists: &'a [PlaylistInfo],
    mut f: impl FnMut(Row<'a>, Rect),
) {
    // Row bands scale with both the DPI and the font-size setting, so a larger
    // font does not crowd the labels; the metric row height only sets the
    // relative growth.
    let scale = scale(dpi);
    let row_h = (ROW_HEIGHT * scale).round() as i32;
    let heading_h = (HEADING_HEIGHT * scale).round() as i32;
    let gap = (SECTION_GAP * scale).round() as i32;
    let pad = (TOP_PAD * scale).round() as i32;
    let mut y = bounds.top + pad;
    for section in SECTIONS {
        f(
            Row::Heading {
                text: section.heading,
                add: false,
            },
            Rect::new(bounds.left, y, bounds.right, y + heading_h),
        );
        y += heading_h;
        for &view in section.views {
            f(
                Row::View(view),
                Rect::new(bounds.left, y, bounds.right, y + row_h),
            );
            y += row_h;
        }
        y += gap;
    }
    f(
        Row::Heading {
            text: PLAYLISTS_HEADING,
            add: true,
        },
        Rect::new(bounds.left, y, bounds.right, y + heading_h),
    );
    y += heading_h;
    for playlist in playlists {
        f(
            Row::Playlist(playlist),
            Rect::new(bounds.left, y, bounds.right, y + row_h),
        );
        y += row_h;
    }
}

/// The client rectangle of the `+` on a playlists heading row.
pub(super) fn add_button_rect(dpi: u32, heading: Rect) -> Rect {
    let size = (ADD_BUTTON_SIZE * scale(dpi)).round() as i32;
    let margin = (ADD_RIGHT_MARGIN * scale(dpi)).round() as i32;
    let right = heading.right - margin;
    let left = right - size;
    let top = heading.top + (heading.height() - size) / 2;
    Rect::new(left, top, right, top + size)
}

/// Device pixels per design unit at `dpi` and the current font scale.
fn scale(dpi: u32) -> f32 {
    dpi as f32 / 96.0 * crate::appearance::font_scale()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playlist(id: u64, name: &str) -> PlaylistInfo {
        PlaylistInfo {
            id,
            name: name.into(),
            track_count: 0,
        }
    }

    #[test]
    fn rows_cover_every_view_then_the_playlists() {
        let lists = [playlist(1, "A"), playlist(2, "B")];
        let mut views = Vec::new();
        let mut headings = Vec::new();
        let mut playlists = Vec::new();
        for_each_row(96, Rect::new(0, 0, 200, 600), &lists, |row, _| match row {
            Row::Heading { text, .. } => headings.push(text.to_owned()),
            Row::View(view) => views.push(view),
            Row::Playlist(info) => playlists.push(info.id),
        });
        let expected: Vec<View> = SECTIONS
            .iter()
            .flat_map(|section| section.views.iter().copied())
            .collect();
        assert_eq!(views, expected);
        assert_eq!(headings.last().map(String::as_str), Some(PLAYLISTS_HEADING));
        assert_eq!(playlists, vec![1, 2]);
    }

    #[test]
    fn the_add_button_sits_inside_the_playlists_heading() {
        let lists = [playlist(1, "A")];
        let mut heading = Rect::default();
        let mut add = true;
        for_each_row(96, Rect::new(0, 0, 200, 600), &lists, |row, rect| {
            if let Row::Heading { text, add: has } = row
                && text == PLAYLISTS_HEADING
            {
                heading = rect;
                add = has;
            }
        });
        assert!(add, "the playlists heading carries the + button");
        let button = add_button_rect(96, heading);
        assert!(heading.left <= button.left && button.right <= heading.right);
        assert!(heading.top <= button.top && button.bottom <= heading.bottom);
        assert!(button.right < heading.right, "it leaves a right margin");
    }

    #[test]
    fn rows_scale_with_the_font_size() {
        use emusic_ui::state::{Appearance, FontSize};

        let bounds = Rect::new(0, 0, 200, 600);
        let music_height = |appearance| {
            crate::appearance::install(appearance, 96);
            let mut height = 0;
            for_each_row(96, bounds, &[], |row, rect| {
                if matches!(row, Row::View(View::Music)) {
                    height = rect.height();
                }
            });
            height
        };
        let default = music_height(Appearance::default());
        let larger = music_height(Appearance {
            font_size: FontSize::Larger,
            ..Appearance::default()
        });
        assert!(
            larger > default,
            "a larger font must give the navigator taller rows ({larger} vs {default})"
        );
        // Leave the thread-local in its default state for the other tests.
        crate::appearance::install(Appearance::default(), 96);
    }
}
