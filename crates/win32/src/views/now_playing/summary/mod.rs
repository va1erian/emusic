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
use emusic_ui::state::Metrics;
use emusic_ui::views::now_playing::{ModuleView, NowPlayingView as Model};
use win32ui::d2d::{D2dCanvas, Font, FontSpec, ImageId, RectF, TextSystem};
use win32ui::gdi::Canvas;
use win32ui::prelude::*;
use win32ui::{Rect, RgbaImage, Size, Theme};

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

/// The summary's fonts, resolved once from DirectWrite.
struct Fonts {
    title: Option<Font>,
    body: Option<Font>,
    small: Option<Font>,
}

impl Fonts {
    /// Points to the device-independent em size DirectWrite takes, keeping the
    /// same sizes the GDI path used (`96 / 72` points per DIP).
    fn dip_font(system: &TextSystem, points: f32, weight: u16) -> Option<Font> {
        system
            .font(&FontSpec::new("Segoe UI", points * 96.0 / 72.0).weight(weight))
            .ok()
    }

    /// Builds the summary's fonts from the appearance metrics (#309): the
    /// title is the metric title size, the body and caption the metric body
    /// and small sizes.
    fn from_metrics(metrics: Metrics) -> Self {
        let Ok(system) = TextSystem::new() else {
            return Self {
                title: None,
                body: None,
                small: None,
            };
        };
        Self {
            title: Self::dip_font(&system, metrics.title, 700),
            body: Self::dip_font(&system, metrics.body, 400),
            small: Self::dip_font(&system, metrics.small, 400),
        }
    }
}

/// The decoded artwork plus its Direct2D image, uploaded on first paint.
struct Artwork {
    image: Rc<RgbaImage>,
    id: Cell<Option<ImageId>>,
}

/// The Direct2D-painted summary widget.
pub(super) struct SummaryWidget {
    data: RefCell<SummaryData>,
    artwork: RefCell<Option<Artwork>>,
    /// The Direct2D image of a replaced artwork, released on the next paint
    /// (the surface is only reachable while painting).
    forget: Cell<Option<ImageId>>,
    /// Clickable regions, refreshed on every paint.
    hits: RefCell<Vec<(Rect, Hit)>>,
    hot: Cell<Option<Hit>>,
    pressed: Cell<Option<Hit>>,
    fonts: RefCell<Fonts>,
}

impl SummaryWidget {
    pub(super) fn new() -> Self {
        Self {
            data: RefCell::new(SummaryData::default()),
            artwork: RefCell::new(None),
            forget: Cell::new(None),
            hits: RefCell::new(Vec::new()),
            hot: Cell::new(None),
            pressed: Cell::new(None),
            fonts: RefCell::new(Fonts::from_metrics(crate::appearance::metrics())),
        }
    }

    /// Rebuilds the summary's fonts from new appearance metrics (#309). New
    /// fonts are created before the old handles are dropped.
    pub(super) fn set_metrics(&mut self, metrics: Metrics) {
        *self.fonts.borrow_mut() = Fonts::from_metrics(metrics);
    }

    /// Rebuilds the metadata snapshot from the shared model.
    pub(super) fn set_model(&mut self, model: &Model) {
        *self.data.borrow_mut() = SummaryData::from_model(model);
    }

    /// Replaces the artwork, returning whether it changed.
    pub(super) fn set_artwork(&mut self, artwork: Option<Rc<RgbaImage>>) -> bool {
        let changed = match (&*self.artwork.borrow(), &artwork) {
            (None, None) => false,
            (Some(previous), Some(next)) => !Rc::ptr_eq(&previous.image, next),
            _ => true,
        };
        if changed {
            if let Some(previous) = self.artwork.borrow_mut().take()
                && let Some(id) = previous.id.get()
            {
                self.forget.set(Some(id));
            }
            *self.artwork.borrow_mut() = artwork.map(|image| Artwork {
                image,
                id: Cell::new(None),
            });
        }
        changed
    }
}

impl CustomWidget for SummaryWidget {
    type Event = SummaryEvent;

    fn preferred_size(&self, dpi: u32) -> Option<Size> {
        Some(Size::new(
            dip(PANEL_WIDTH).to_px(dpi).value(),
            dip(ARTWORK_EDGE + 260.0).to_px(dpi).value(),
        ))
    }

    fn renderer(&self) -> Renderer {
        Renderer::Direct2D
    }

    fn paint(&self, _canvas: &Canvas, _bounds: Rect, _theme: &Theme) {}

    fn paint_d2d(&self, canvas: &mut D2dCanvas<'_>, bounds: RectF, theme: &Theme) {
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
