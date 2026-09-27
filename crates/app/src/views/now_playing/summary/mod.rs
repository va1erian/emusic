//! The now-playing summary: artwork, metadata and tracker-module info painted
//! into one portable [`Control`] (#110, #247).
//!
//! All display strings come from the shared
//! [`emusic_ui::views::now_playing::NowPlayingView`] model. This module owns the
//! widget's data snapshot; [`paint`](paint) lays the parts out and draws them
//! through the portable `Canvas`. Clicks on the star, the artist/album links
//! and the two action links are mapped back to [`SummaryEvent`]s from the hit
//! regions recorded on the last paint.

mod paint;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use emusic_ui::library_api::TrackInfo;
use emusic_ui::views::now_playing::{ModuleView, NowPlayingView as Model};
use xui::xui_core::Image;
use xui::xui_core::app::Ui;
use xui::xui_core::backend::{Event, NodeKind, NodeSpec, WidgetId};
use xui::xui_core::geometry::{Point, Rect};
use xui::xui_core::widget::Control;

use crate::app::Msg;

use paint::paint;

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
}

/// A clickable region of the summary, as laid out by the last paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Hit {
    Star,
    Artist,
    Album,
    Properties,
    EditTags,
}

impl Hit {
    fn event(self) -> SummaryEvent {
        match self {
            Hit::Star => SummaryEvent::ToggleStar,
            Hit::Artist => SummaryEvent::GoToArtist,
            Hit::Album => SummaryEvent::GoToAlbum,
            Hit::Properties => SummaryEvent::ShowProperties,
            Hit::EditTags => SummaryEvent::EditTags,
        }
    }
}

/// The display snapshot the widget paints, rebuilt when the model revision
/// changes.
#[derive(Default, Clone)]
pub(super) struct SummaryData {
    pub(super) playing: bool,
    pub(super) has_track: bool,
    pub(super) title: String,
    pub(super) artist: String,
    /// The album name; empty disables the album link.
    pub(super) album: String,
    /// The full album line (`Album (year) · Track N · Genre`).
    pub(super) album_line: String,
    pub(super) details: Option<String>,
    pub(super) path: String,
    pub(super) starred: bool,
    pub(super) module: Option<ModuleView>,
}

impl SummaryData {
    fn from_model(model: &Model) -> Self {
        let now = model.now_playing();
        let track = model.track();
        let path = track
            .map(|t| t.path.clone())
            .or_else(|| now.map(|np| np.path.clone()))
            .unwrap_or_default();
        Self {
            playing: model.is_playing(),
            has_track: track.is_some(),
            title: now.map(|np| np.title.clone()).unwrap_or_default(),
            artist: now.map(|np| np.artist.clone()).unwrap_or_default(),
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

/// The owner-drawn summary control.
pub struct Summary {
    ui: Ui<Msg>,
    control: Control<Msg>,
    data: Rc<RefCell<SummaryData>>,
    artwork: Rc<RefCell<Option<Image>>>,
}

impl Summary {
    /// Creates the empty summary; [`sync`](Self::sync) fills it.
    pub fn new(ui: &Ui<Msg>) -> Summary {
        let control = Control::new(ui, &NodeSpec::new(NodeKind::Custom, Rect::default()))
            .expect("create now-playing summary");
        let data = Rc::new(RefCell::new(SummaryData::default()));
        let artwork = Rc::new(RefCell::new(None));
        let hits = Rc::new(RefCell::new(Vec::new()));
        let hot = Rc::new(Cell::new(None));
        {
            let data = Rc::clone(&data);
            let artwork = Rc::clone(&artwork);
            let hits = Rc::clone(&hits);
            let hot = Rc::clone(&hot);
            let theme = ui.theme_handle();
            let painter_ui = ui.clone();
            control.set_painter(Rc::new(move |canvas| {
                paint(
                    canvas,
                    &painter_ui,
                    &theme.get(),
                    &data.borrow(),
                    artwork.borrow().as_ref(),
                    &mut hits.borrow_mut(),
                    hot.get(),
                );
            }));
        }
        {
            let hits = Rc::clone(&hits);
            let hot = Rc::clone(&hot);
            let event_ui = ui.clone();
            let id = control.id();
            control.on_events(move |event| input(event, &event_ui, id, &hits, &hot));
        }
        Summary {
            ui: ui.clone(),
            control,
            data,
            artwork,
        }
    }

    /// Rebuilds the display snapshot from the shared model.
    pub fn sync(&mut self, model: &Model) {
        *self.data.borrow_mut() = SummaryData::from_model(model);
        self.control.invalidate();
    }

    /// Replaces the artwork, invalidating when it changed.
    pub fn set_artwork(&mut self, image: Option<Image>) {
        let changed = match (&*self.artwork.borrow(), &image) {
            (None, None) => false,
            (Some(previous), Some(next)) => previous.id() != next.id(),
            _ => true,
        };
        if changed {
            *self.artwork.borrow_mut() = image;
            self.control.invalidate();
        } else if image.is_some() {
            *self.artwork.borrow_mut() = image;
        }
    }

    /// Moves/resizes the summary.
    pub fn set_bounds(&self, rect: Rect) {
        self.ui.apply_moves(&[(self.control.id(), rect)]);
    }

    /// Shows or hides the summary.
    pub fn set_visible(&self, visible: bool) {
        self.ui.set_visible(self.control.id(), visible);
    }
}

/// The clickable region under a client point, if any.
fn hit(hits: &RefCell<Vec<(Rect, Hit)>>, x: i32, y: i32) -> Option<Hit> {
    hits.borrow()
        .iter()
        .find(|(rect, _)| rect.contains(Point::new(x, y)))
        .map(|(_, hit)| *hit)
}

/// Maps one pointer event to the app message it raises, tracking the hover.
fn input(
    event: &Event,
    ui: &Ui<Msg>,
    id: WidgetId,
    hits: &RefCell<Vec<(Rect, Hit)>>,
    hot: &Cell<Option<Hit>>,
) -> Option<Msg> {
    match *event {
        Event::MouseMove { x, y, .. } => {
            let found = hit(hits, x, y);
            if found != hot.get() {
                hot.set(found);
                ui.invalidate(id);
            }
            None
        }
        Event::MouseLeave => {
            if hot.take().is_some() {
                ui.invalidate(id);
            }
            None
        }
        Event::MouseUp {
            x,
            y,
            button: xui::xui_core::MouseButton::Left,
            ..
        } => hit(hits, x, y).map(|hit| Msg::NowPlayingSummary(hit.event())),
        _ => None,
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
