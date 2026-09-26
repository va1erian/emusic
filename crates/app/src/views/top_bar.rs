//! The top region (#375): the caption drag band plus the transport band, on the
//! portable [`TopBar`].
//!
//! All transport state and formatting come from
//! [`emusic_ui::panels::top_bar::TopBar`]; this view only lays the portable
//! widget out and maps its events onto the shell's existing [`Msg`]s. The
//! search box is a portable [`Edit`] whose text mirrors
//! [`AppState::search_query`](emusic_ui::state::AppState::search_query).
//!
//! The caption band above the transport band is a transparent drag region on
//! platforms that draw their own chrome; on Windows the native extended title
//! bar owns the strip, so the band node is kept hidden.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use emusic_ui::panels::top_bar::TopBar as TopBarModel;
use emusic_ui::state::Command;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Edit, Glyph, HasText, Label, TopBar, TopBarId};

use crate::app::Msg;
use crate::window::WindowChrome;

/// Width of the search field, in device-independent pixels.
const SEARCH_WIDTH: f32 = 200.0;
/// Height of the search field, in device-independent pixels.
const SEARCH_HEIGHT: f32 = 22.0;
/// Horizontal inset of the band contents from the window edges.
const BAND_INSET: f32 = 6.0;
/// A spinner/transport glyph's text size is handled by the widget; these are
/// the short marks drawn as icons.
const GLYPH_PREVIOUS: &str = "\u{25C0}\u{25C0}";
const GLYPH_PLAY: &str = "\u{25B6}";
const GLYPH_PAUSE: &str = "\u{23F8}";
const GLYPH_STOP: &str = "\u{25A0}";
const GLYPH_NEXT: &str = "\u{25B6}\u{25B6}";
const GLYPH_REPEAT: &str = "\u{27F3}";
const GLYPH_SHUFFLE: &str = "\u{21C4}";
const GLYPH_MINIMIZE: &str = "\u{2013}";
const GLYPH_MAXIMIZE: &str = "\u{25A1}";

/// Opaque item handles; the ids are the view's private numbering.
const PREVIOUS: TopBarId = TopBarId::new(1);
const PLAY_PAUSE: TopBarId = TopBarId::new(2);
const STOP: TopBarId = TopBarId::new(3);
const NEXT: TopBarId = TopBarId::new(4);
const ELAPSED: TopBarId = TopBarId::new(5);
const SEEK: TopBarId = TopBarId::new(6);
const TOTAL: TopBarId = TopBarId::new(7);
const REPEAT: TopBarId = TopBarId::new(8);
const SHUFFLE: TopBarId = TopBarId::new(9);
const VOLUME: TopBarId = TopBarId::new(10);
const MINIMIZE: TopBarId = TopBarId::new(11);
const MAXIMIZE: TopBarId = TopBarId::new(12);
const CLOSE: TopBarId = TopBarId::new(13);
const SEARCH: TopBarId = TopBarId::new(14);

/// The top region: the caption drag band and the transport band.
pub struct TopBarView {
    ui: Ui<Msg>,
    caption: Label<Msg>,
    bar: TopBar<Msg>,
    search: Edit<Msg>,
    /// Whether the play/pause glyph is currently the pause icon; a change
    /// rebuilds the bar (the portable bar has no icon setter).
    playing: bool,
    /// The current track length, read by the seek mapper to turn the slider's
    /// `0..=1` fraction into seconds.
    duration: Rc<Cell<f64>>,
    caption_bounds: Rect,
    bar_bounds: Rect,
}

impl TopBarView {
    /// Builds the caption band, the transport bar and the search box.
    pub fn new(ui: &Ui<Msg>) -> TopBarView {
        let caption = Label::new(ui, Rect::default(), "").expect("create caption band");
        let duration = Rc::new(Cell::new(0.0));
        let bar = build_bar(ui, false, Rc::clone(&duration));
        let search = Edit::new(ui, Rect::default(), "")
            .expect("create search box")
            .on_change(|text| Some(Msg::Dispatch(Command::SetSearchQuery(text.to_string()))));
        // On Windows the native extended title bar owns the strip; elsewhere
        // the band is the app's drag region.
        ui.set_visible(caption.id(), !cfg!(windows));
        TopBarView {
            ui: ui.clone(),
            caption,
            bar,
            search,
            playing: false,
            duration,
            caption_bounds: Rect::default(),
            bar_bounds: Rect::default(),
        }
    }

    /// Marks the caption band as the window's drag region.
    pub fn apply_chrome(&self, chrome: &WindowChrome) {
        chrome.set_drag_region(self.caption.id(), true);
    }

    /// Positions the caption band, the transport band and the search box.
    pub fn set_bounds(&mut self, caption: Rect, bar: Rect) {
        self.caption_bounds = caption;
        let dpi = self.ui.dpi();
        let search_w = dip(SEARCH_WIDTH).to_px(dpi).value();
        let search_h = dip(SEARCH_HEIGHT).to_px(dpi).value();
        let inset = dip(BAND_INSET).to_px(dpi).value();
        let bar_right = (bar.right - search_w - inset).max(bar.left);
        self.bar_bounds = Rect::new(bar.left, bar.top, bar_right, bar.bottom);
        let edit_top = (bar.top + bar.bottom - search_h) / 2;
        let edit = Rect::new(
            bar.right - search_w - inset,
            edit_top,
            bar.right - inset,
            edit_top + search_h,
        );
        self.ui.apply_moves(&[
            (self.caption.id(), caption),
            (self.bar.id(), self.bar_bounds),
            (self.search.id(), edit),
        ]);
    }

    /// Pushes the model's transport state into the band and mirrors the search
    /// query into the portable field.
    pub fn sync(&mut self, model: &TopBarModel, search_query: &str) {
        if self.playing != model.is_playing() {
            self.playing = model.is_playing();
            self.rebuild();
        }
        let duration = model.duration_secs().unwrap_or(0.0);
        self.duration.set(duration);
        let fraction = if duration > 0.0 {
            model.position_secs() / duration
        } else {
            0.0
        };
        self.bar.set_value(SEEK, fraction.clamp(0.0, 1.0));
        self.bar.set_enabled(SEEK, model.seek_supported());
        self.bar.set_text(ELAPSED, &model.elapsed_text());
        self.bar
            .set_text(TOTAL, &model.total_text().unwrap_or_default());
        self.bar.set_value(VOLUME, f64::from(model.volume()));
        self.bar.set_checked(REPEAT, model.repeat_active());
        self.bar.set_checked(SHUFFLE, model.shuffle());

        if self.search.text() != search_query {
            self.search.set_text(search_query);
        }
    }

    /// Recreates the transport bar (only the play/pause glyph is not mutable in
    /// place) and restores its bounds.
    fn rebuild(&mut self) {
        self.bar = build_bar(&self.ui, self.playing, Rc::clone(&self.duration));
        self.ui.apply_moves(&[(self.bar.id(), self.bar_bounds)]);
    }
}

/// Builds the transport bar for the given play state.
fn build_bar(ui: &Ui<Msg>, playing: bool, duration: Rc<Cell<f64>>) -> TopBar<Msg> {
    let play = if playing {
        Glyph::Text(GLYPH_PAUSE)
    } else {
        Glyph::Text(GLYPH_PLAY)
    };
    let bar = TopBar::new(ui, Rect::default())
        .expect("create top bar")
        .icon(PREVIOUS, Glyph::Text(GLYPH_PREVIOUS))
        .tooltip(PREVIOUS, "Previous")
        .icon(PLAY_PAUSE, play)
        .tooltip(PLAY_PAUSE, if playing { "Pause" } else { "Play" })
        .icon(STOP, Glyph::Text(GLYPH_STOP))
        .tooltip(STOP, "Stop")
        .icon(NEXT, Glyph::Text(GLYPH_NEXT))
        .tooltip(NEXT, "Next")
        .spacer_weight(2)
        .label(ELAPSED, "0:00")
        .slider(SEEK, 0.0, 1.0)
        .tooltip(SEEK, "Seek")
        .label(TOTAL, "")
        .spacer_weight(2)
        .toggle(REPEAT, Glyph::Text(GLYPH_REPEAT))
        .tooltip(REPEAT, "Repeat")
        .toggle(SHUFFLE, Glyph::Text(GLYPH_SHUFFLE))
        .tooltip(SHUFFLE, "Shuffle")
        .spacer_weight(2)
        .slider(VOLUME, 0.0, 1.0)
        .tooltip(VOLUME, "Volume")
        .icon(SEARCH, Glyph::Search)
        .tooltip(SEARCH, "Search the library");

    // A platform whose backend draws no native window buttons gets portable
    // ones at the trailing edge.
    let bar = if cfg!(windows) {
        bar
    } else {
        bar.spacer()
            .icon(MINIMIZE, Glyph::Text(GLYPH_MINIMIZE))
            .tooltip(MINIMIZE, "Minimize")
            .icon(MAXIMIZE, Glyph::Text(GLYPH_MAXIMIZE))
            .tooltip(MAXIMIZE, "Maximize")
            .icon(CLOSE, Glyph::Close)
            .tooltip(CLOSE, "Close")
    };

    bar.on_click(move |id| {
        if id == PREVIOUS {
            Some(Msg::Dispatch(Command::PlayerPrevious))
        } else if id == PLAY_PAUSE {
            Some(Msg::Dispatch(Command::PlayerPlayPause))
        } else if id == STOP {
            Some(Msg::Dispatch(Command::PlayerStop))
        } else if id == NEXT {
            Some(Msg::Dispatch(Command::PlayerNext))
        } else if id == MINIMIZE {
            Some(Msg::Minimize)
        } else if id == MAXIMIZE {
            Some(Msg::ToggleMaximize)
        } else if id == CLOSE {
            Some(Msg::Quit)
        } else {
            None
        }
    })
    .on_toggle(|id, _checked| {
        if id == REPEAT {
            Some(Msg::Dispatch(Command::PlayerToggleRepeat))
        } else if id == SHUFFLE {
            Some(Msg::Dispatch(Command::PlayerToggleShuffle))
        } else {
            None
        }
    })
    .on_change(move |id, value| {
        if id == SEEK {
            let duration = duration.get();
            (duration > 0.0).then(|| {
                Msg::Dispatch(Command::PlayerSeek(Duration::from_secs_f64(
                    value * duration,
                )))
            })
        } else if id == VOLUME {
            Some(Msg::Dispatch(Command::PlayerSetVolume(value as f32)))
        } else {
            None
        }
    })
}
