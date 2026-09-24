//! The now-playing panel's summary: artwork, metadata and tracker-module info,
//! drawn into one owner-drawn child window (#110).
//!
//! All display strings come from the shared
//! [`emusic_ui::views::now_playing::NowPlayingView`] model. This module owns the
//! widget and its data snapshot; the submodules lay the parts out
//! ([`layout`]), paint them from semantic theme tokens ([`draw`]) and map
//! clicks to [`SummaryEvent`]s ([`input`]).

mod draw;
mod input;
mod layout;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use emusic_ui::library_api::TrackInfo;
use emusic_ui::views::now_playing::{ModuleView, NowPlayingView as Model};
use win32ui::gdi::{Bitmap, Canvas, Font, FontWeight};
use win32ui::prelude::*;
use win32ui::{Rect, Size, Theme};

use super::PANEL_WIDTH;
use input::Hit;

/// Artwork box edge, in device-independent pixels.
pub(super) const ARTWORK_EDGE: f32 = 200.0;
/// Padding around the panel content.
const PAD: f32 = 8.0;
/// Gap between blocks.
const GAP: f32 = 8.0;
/// A regular text line's height.
const LINE: f32 = 18.0;
/// Height of the star + title row.
const TITLE_LINE: f32 = 22.0;
/// Width of the star hit box.
const STAR: f32 = 20.0;
/// Small gap between the star and the title, or between links.
const LINK_GAP: f32 = 6.0;

/// The artwork bitmap edge for `dpi`, in pixels (at least one).
pub(super) fn artwork_edge_px(dpi: u32) -> u32 {
    dip(ARTWORK_EDGE).to_px(dpi).value().max(1) as u32
}

/// What a click on the summary asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryEvent {
    /// Flip the playing track's starred flag.
    ToggleStar,
    /// Jump to the Artists view for the playing artist.
    GoToArtist,
    /// Jump to the Albums view for the playing album.
    GoToAlbum,
    /// Open the track's Properties dialog.
    ShowProperties,
    /// Open the tag editor for the playing track.
    EditTags,
    /// Reveal the playing file in Explorer.
    OpenFolder,
}

/// The display snapshot the widget paints, rebuilt whenever the model's
/// revision changes.
#[derive(Default, Clone)]
struct SummaryData {
    playing: bool,
    has_track: bool,
    title: String,
    artist: String,
    /// Album name, the target of the album link.
    album: String,
    /// The full album line (`Album (year) · Track N · Genre`).
    album_line: String,
    details: Option<String>,
    path: String,
    starred: bool,
    module: Option<ModuleView>,
}

impl SummaryData {
    fn from_model(model: &Model) -> Self {
        let now = model.now_playing();
        let track = model.track();
        let title = now.map(|np| np.title.clone()).unwrap_or_default();
        let artist = now.map(|np| np.artist.clone()).unwrap_or_default();
        let path = track
            .map(|t| t.path.clone())
            .or_else(|| now.map(|np| np.path.clone()))
            .unwrap_or_default();
        Self {
            playing: model.is_playing(),
            has_track: track.is_some(),
            title,
            artist,
            album: track.map(|t| t.album.clone()).unwrap_or_default(),
            album_line: track
                .map(|t| album_line(t, model.album_year()))
                .unwrap_or_default(),
            details: model.details_text(),
            path,
            starred: track.is_some_and(|t| t.starred),
            module: model.module(),
        }
    }
}

/// The `Album (year) · Track N · Genre` line shown under the artist.
fn album_line(track: &TrackInfo, year: Option<u32>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !track.album.is_empty() {
        parts.push(match year {
            Some(year) => format!("{} ({year})", track.album),
            None => track.album.clone(),
        });
    }
    if let Some(no) = track.track_no {
        let mut text = format!("Track {no}");
        if let Some(disc) = track.disc_no {
            text.push_str(&format!(", Disc {disc}"));
        }
        parts.push(text);
    }
    if !track.genre.is_empty() {
        parts.push(track.genre.clone());
    }
    parts.join("  ·  ")
}

/// The summary's fonts, created once per DPI.
struct Fonts {
    title: Option<Font>,
    body: Option<Font>,
    small: Option<Font>,
}

impl Fonts {
    fn new(dpi: u32) -> Self {
        Self {
            title: Font::new("Segoe UI", 12.0, FontWeight::Bold, dpi).ok(),
            body: Font::new("Segoe UI", 9.75, FontWeight::Regular, dpi).ok(),
            small: Font::new("Segoe UI", 8.5, FontWeight::Regular, dpi).ok(),
        }
    }
}

/// The owner-drawn summary widget.
pub(super) struct SummaryWidget {
    dpi: Cell<u32>,
    data: RefCell<SummaryData>,
    artwork: RefCell<Option<Rc<Bitmap>>>,
    /// Clickable regions, refreshed on every paint.
    hits: RefCell<Vec<(Rect, Hit)>>,
    hot: Cell<Option<Hit>>,
    pressed: Cell<Option<Hit>>,
    fonts: RefCell<Fonts>,
}

impl SummaryWidget {
    pub(super) fn new(dpi: u32) -> Self {
        Self {
            dpi: Cell::new(dpi),
            data: RefCell::new(SummaryData::default()),
            artwork: RefCell::new(None),
            hits: RefCell::new(Vec::new()),
            hot: Cell::new(None),
            pressed: Cell::new(None),
            fonts: RefCell::new(Fonts::new(dpi)),
        }
    }

    /// Rebuilds the metadata snapshot from the shared model.
    pub(super) fn set_model(&mut self, model: &Model) {
        *self.data.borrow_mut() = SummaryData::from_model(model);
    }

    /// Replaces the artwork bitmap, returning whether it changed.
    pub(super) fn set_artwork(&mut self, artwork: Option<Rc<Bitmap>>) -> bool {
        let changed = match (&*self.artwork.borrow(), &artwork) {
            (None, None) => false,
            (Some(previous), Some(next)) => !Rc::ptr_eq(previous, next),
            _ => true,
        };
        *self.artwork.borrow_mut() = artwork;
        changed
    }
}

impl CustomWidget for SummaryWidget {
    type Event = SummaryEvent;

    fn preferred_size(&self, dpi: u32) -> Option<Size> {
        self.dpi.set(dpi);
        Some(Size::new(
            dip(PANEL_WIDTH).to_px(dpi).value(),
            dip(ARTWORK_EDGE + 260.0).to_px(dpi).value(),
        ))
    }

    fn paint(&self, canvas: &Canvas, bounds: Rect, theme: &Theme) {
        draw::paint(self, canvas, bounds, theme);
    }

    fn input(&self, input: Input, cx: &mut WidgetCx<Self::Event>) {
        input::handle(self, input, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn album_line_joins_the_tagged_parts() {
        let track = TrackInfo {
            album: "Signal".to_string(),
            track_no: Some(3),
            disc_no: Some(1),
            genre: "Electronic".to_string(),
            ..TrackInfo::default()
        };
        assert_eq!(
            album_line(&track, Some(1999)),
            "Signal (1999)  ·  Track 3, Disc 1  ·  Electronic"
        );
    }

    #[test]
    fn album_line_is_empty_without_tags() {
        assert_eq!(album_line(&TrackInfo::default(), None), "");
    }
}
