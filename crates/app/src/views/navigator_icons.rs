//! Rasterized [Lucide](https://lucide.dev) icons for the navigator rows (#442).
//!
//! The rows used to draw a hand-approximated portable [`Glyph`], which the
//! software (canvas) backend rendered as rough shapes. They now carry a
//! bitmap: the equivalent Lucide line icon, rasterized to the row's icon slot
//! and embedded with [`include_bytes!`]. `TreeRow::icon` already paints an
//! [`Image`].
//!
//! Lucide is ISC-licensed; see [`THIRD-PARTY-NOTICES.md`](../../../THIRD-PARTY-NOTICES.md)
//! and `crates/app/assets/navigator/README.txt`. The PNGs are generated from
//! the vendored Lucide `1.48.0` SVGs by `scripts/gen-navigator-icons.mjs`, at
//! 2x the 16-dip slot (32 px) so they stay crisp on HiDPI and downscale
//! cleanly at 100%. An image paints in its own colours, so each icon has a
//! light and a dark raster; the navigator re-tints when the theme changes.
//!
//! Every icon is decoded once ([`image`] clones a cached [`Image`]) and shared
//! by every row, so a repaint is one cache lookup per row, never a decode.
//!
//! [`Glyph`]: xui::xui_core::widget::Glyph

use std::sync::LazyLock;

use emusic_ui::state::View;
use xui::xui_core::image::Image;

/// The rasterized edge, in pixels: 2x the tree's 16-dip row icon slot.
#[cfg(test)]
pub(crate) const SIZE: u32 = 32;

/// The Lucide source backing a view: its icon file name and whether it is
/// drawn filled. `Starred` and `Most Played` share `star`, outlined and
/// filled respectively.
///
/// Only the tests read this back; the embedded PNGs are the source of truth
/// at runtime.
#[cfg(test)]
pub(crate) fn source(view: View) -> (&'static str, bool) {
    match view {
        View::Music => ("music", false),
        View::Albums => ("disc-3", false),
        View::Artists => ("users", false),
        View::Genres => ("tags", false),
        View::Folders => ("folder", false),
        View::Starred => ("star", false),
        View::MostPlayed => ("star", true),
        View::History => ("history", false),
        View::NowPlaying => ("play", false),
        View::Visualization => ("monitor", false),
        View::Settings => ("settings", false),
    }
}

/// The icon for `view` at the given theme; the cached [`Image`] is cloned, so
/// all rows share one decode and the backend caches one upload.
pub(crate) fn image(view: View, dark: bool) -> Image {
    let entry = ICONS
        .iter()
        .find(|entry| entry.view == view)
        .expect("every view has a navigator icon");
    if dark {
        entry.dark.clone()
    } else {
        entry.light.clone()
    }
}

/// One view's two theme rasters, decoded once at first use.
struct Entry {
    view: View,
    light: Image,
    dark: Image,
}

/// The light and dark rasters for every view, in [`View::ALL`] order.
static ICONS: LazyLock<Vec<Entry>> = LazyLock::new(|| {
    View::ALL
        .iter()
        .map(|&view| Entry {
            view,
            light: decode(light(view)),
            dark: decode(dark(view)),
        })
        .collect()
});

/// Decodes one embedded, generated PNG.
fn decode(bytes: &[u8]) -> Image {
    Image::decode(bytes).expect("an embedded navigator PNG decodes")
}

/// The light-theme PNG for `view`, at the light theme's text colour.
fn light(view: View) -> &'static [u8] {
    match view {
        View::Music => include_bytes!("../../assets/navigator/light/music.png"),
        View::Albums => include_bytes!("../../assets/navigator/light/albums.png"),
        View::Artists => include_bytes!("../../assets/navigator/light/artists.png"),
        View::Genres => include_bytes!("../../assets/navigator/light/genres.png"),
        View::Folders => include_bytes!("../../assets/navigator/light/folders.png"),
        View::Starred => include_bytes!("../../assets/navigator/light/starred.png"),
        View::MostPlayed => include_bytes!("../../assets/navigator/light/most-played.png"),
        View::History => include_bytes!("../../assets/navigator/light/history.png"),
        View::NowPlaying => include_bytes!("../../assets/navigator/light/now-playing.png"),
        View::Visualization => include_bytes!("../../assets/navigator/light/visualization.png"),
        View::Settings => include_bytes!("../../assets/navigator/light/settings.png"),
    }
}

/// The dark-theme PNG for `view`, at the dark theme's text colour.
fn dark(view: View) -> &'static [u8] {
    match view {
        View::Music => include_bytes!("../../assets/navigator/dark/music.png"),
        View::Albums => include_bytes!("../../assets/navigator/dark/albums.png"),
        View::Artists => include_bytes!("../../assets/navigator/dark/artists.png"),
        View::Genres => include_bytes!("../../assets/navigator/dark/genres.png"),
        View::Folders => include_bytes!("../../assets/navigator/dark/folders.png"),
        View::Starred => include_bytes!("../../assets/navigator/dark/starred.png"),
        View::MostPlayed => include_bytes!("../../assets/navigator/dark/most-played.png"),
        View::History => include_bytes!("../../assets/navigator/dark/history.png"),
        View::NowPlaying => include_bytes!("../../assets/navigator/dark/now-playing.png"),
        View::Visualization => include_bytes!("../../assets/navigator/dark/visualization.png"),
        View::Settings => include_bytes!("../../assets/navigator/dark/settings.png"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_view_maps_to_its_lucide_icon() {
        assert_eq!(source(View::Music).0, "music");
        assert_eq!(source(View::Albums).0, "disc-3");
        assert_eq!(source(View::Artists).0, "users");
        assert_eq!(source(View::Genres).0, "tags");
        assert_eq!(source(View::Folders).0, "folder");
        assert_eq!(source(View::Starred).0, "star");
        assert_eq!(source(View::MostPlayed).0, "star");
        assert_eq!(source(View::History).0, "history");
        assert_eq!(source(View::NowPlaying).0, "play");
        assert_eq!(source(View::Visualization).0, "monitor");
        assert_eq!(source(View::Settings).0, "settings");
    }

    #[test]
    fn only_most_played_is_filled() {
        assert!(
            source(View::MostPlayed).1,
            "Most Played reuses the star, filled"
        );
        for &view in &View::ALL {
            if view != View::MostPlayed {
                assert!(!source(view).1, "{view:?} stays an outline icon");
            }
        }
    }

    #[test]
    fn every_view_decodes_a_themed_non_blank_icon() {
        for &view in &View::ALL {
            let light = image(view, false);
            let dark = image(view, true);
            assert_eq!(light.size(), (SIZE, SIZE), "{view:?}");
            assert_eq!(dark.size(), (SIZE, SIZE), "{view:?}");
            assert_ne!(light, dark, "{view:?} is tinted per theme");
            assert!(
                light.pixels().iter().any(|&channel| channel != 0),
                "{view:?} light icon is blank"
            );
            assert!(
                dark.pixels().iter().any(|&channel| channel != 0),
                "{view:?} dark icon is blank"
            );
        }
    }
}
