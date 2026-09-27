//! The top region (#375): the caption drag band plus the transport band, on the
//! portable [`TopBar`].
//!
//! All transport state and formatting come from
//! [`emusic_ui::panels::top_bar::TopBar`]; this view only lays the portable
//! widget out and maps its events onto the shell's existing [`Msg`]s. The
//! search box is a portable [`Edit`] whose text mirrors
//! [`AppState::search_query`](emusic_ui::state::AppState::search_query).
//!
//! When the active backend provides no native window chrome
//! ([`crate::backend::has_native_chrome()`]) the caption band above the
//! transport band carries the app title and the portable
//! minimize/maximize/close buttons at its trailing edge, and the band's empty
//! area stays the window's drag region. The native Win32 backend and macOS
//! (native title bar and traffic lights) provide their own chrome, so the band
//! stays hidden and the transport bar is unchanged.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use emusic_ui::panels::top_bar::TopBar as TopBarModel;
use emusic_ui::state::Command;
use xui::xui_core::app::Ui;
use xui::xui_core::geometry::Rect;
use xui::xui_core::units::dip;
use xui::xui_core::widget::{Edit, Glyph, HasText, TopBar, TopBarId};

use crate::app::Msg;
use crate::backend::has_native_chrome;
use crate::window::WindowChrome;

/// Width of the search field, in device-independent pixels.
const SEARCH_WIDTH: f32 = 200.0;
/// Height of the search field, in device-independent pixels.
const SEARCH_HEIGHT: f32 = 22.0;
/// Horizontal inset of the band contents from the window edges.
const BAND_INSET: f32 = 6.0;
/// The app title shown at the caption band's leading edge.
const CAPTION_TITLE: &str = "emusic";
/// Design width of one caption band button. It mirrors the portable `TopBar`'s
/// item width (`xui_core`'s `topbar::items::ITEM`), so the three window buttons
/// tile their band without a gap.
const CAPTION_BUTTON: f32 = 36.0;
/// Design width of the trailing window-button group.
const CAPTION_BUTTONS: f32 = 3.0 * CAPTION_BUTTON;
/// The transport/band marks use the portable [`Glyph`] transport variants, so
/// the same Fluent icons the Win32 build drew render on every backend.
/// Window buttons on platforms that draw no native chrome; Fluent has no
/// minimize/maximize pair that matches the native caption at this size, so keep
/// the simple short marks.
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
const TITLE: TopBarId = TopBarId::new(15);

/// The top region: the caption band and the transport band.
pub struct TopBarView {
    ui: Ui<Msg>,
    /// The caption band: the app title, with the rest of the band as the
    /// window's drag region. Shown only on the canvas backend.
    caption: TopBar<Msg>,
    /// The caption band's trailing window buttons. Shown only on the canvas
    /// backend, as a node of its own so a click reaches the button instead of
    /// the drag region it sits on.
    caption_buttons: TopBar<Msg>,
    /// The band's background surface, behind the transport and the search, so
    /// the one-line search field's column shares the band's colour.
    band: TopBar<Msg>,
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
    /// Whether this backend has no native chrome, so the view draws (and makes
    /// draggable) the custom caption band.
    custom_caption: bool,
}

impl TopBarView {
    /// Builds the caption band, the transport bar and the search box.
    pub fn new(ui: &Ui<Msg>) -> TopBarView {
        let caption = build_caption(ui);
        let caption_buttons = build_caption_buttons(ui);
        // The band's surface spans the whole transport band, behind the
        // transport bar and the search field; it is raised below them.
        let band = TopBar::new(ui, Rect::default()).expect("create band");
        let duration = Rc::new(Cell::new(0.0));
        let bar = build_bar(ui, false, Rc::clone(&duration));
        let search = Edit::new(ui, Rect::default(), "")
            .expect("create search box")
            .on_change(|text| Some(Msg::Dispatch(Command::SetSearchQuery(text.to_string()))));
        // A backend with no native chrome (the canvas backend on Windows/Linux)
        // shows the caption band and its buttons and drags the window from the
        // band; the native Win32 backend and macOS own the strip and hide them.
        let custom_caption = !has_native_chrome();
        ui.set_visible(caption.id(), custom_caption);
        ui.set_visible(caption_buttons.id(), custom_caption);
        ui.raise(bar.id());
        ui.raise(search.id());
        ui.raise(caption_buttons.id());
        TopBarView {
            ui: ui.clone(),
            caption,
            caption_buttons,
            band,
            bar,
            search,
            playing: false,
            duration,
            caption_bounds: Rect::default(),
            bar_bounds: Rect::default(),
            custom_caption,
        }
    }

    /// Marks the caption band as the window's drag region when the view draws
    /// it. The trailing window buttons are a separate node above it, so they
    /// still receive their clicks. With native chrome the system title bar
    /// already drags the window, so nothing is marked.
    pub fn apply_chrome(&self, chrome: &WindowChrome) {
        if self.custom_caption {
            chrome.set_drag_region(self.caption.id(), true);
        }
    }

    /// Positions the caption band, the transport band and the search box.
    pub fn set_bounds(&mut self, caption: Rect, bar: Rect) {
        self.caption_bounds = caption;
        let dpi = self.ui.dpi();
        let search_w = dip(SEARCH_WIDTH).to_px(dpi).value();
        let search_h = dip(SEARCH_HEIGHT).to_px(dpi).value();
        let inset = dip(BAND_INSET).to_px(dpi).value();
        let buttons_w = dip(CAPTION_BUTTONS).to_px(dpi).value();
        let buttons = Rect::new(
            (caption.right - buttons_w).max(caption.left),
            caption.top,
            caption.right,
            caption.bottom,
        );
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
            (self.caption_buttons.id(), buttons),
            (self.band.id(), bar),
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
        // The recreated bar must sit above the band's surface.
        self.ui.raise(self.bar.id());
    }
}

/// Builds the caption band: the app title at the leading edge. The rest of the
/// band is left empty so it is the window's drag region.
fn build_caption(ui: &Ui<Msg>) -> TopBar<Msg> {
    TopBar::new(ui, Rect::default())
        .expect("create caption band")
        .label(TITLE, CAPTION_TITLE)
}

/// Builds the caption band's trailing window buttons (canvas backend only).
fn build_caption_buttons(ui: &Ui<Msg>) -> TopBar<Msg> {
    TopBar::new(ui, Rect::default())
        .expect("create caption buttons")
        .icon(MINIMIZE, Glyph::Text(GLYPH_MINIMIZE))
        .tooltip(MINIMIZE, "Minimize")
        .icon(MAXIMIZE, Glyph::Text(GLYPH_MAXIMIZE))
        .tooltip(MAXIMIZE, "Maximize")
        .icon(CLOSE, Glyph::Close)
        .tooltip(CLOSE, "Close")
        .on_click(|id| {
            if id == MINIMIZE {
                Some(Msg::Minimize)
            } else if id == MAXIMIZE {
                Some(Msg::ToggleMaximize)
            } else if id == CLOSE {
                Some(Msg::Quit)
            } else {
                None
            }
        })
}

/// Builds the transport bar for the given play state.
fn build_bar(ui: &Ui<Msg>, playing: bool, duration: Rc<Cell<f64>>) -> TopBar<Msg> {
    let play = if playing { Glyph::Pause } else { Glyph::Play };
    // Transport, repeat and shuffle stay together on the left, as in the Win32
    // build; a flexible spacer on each side of the elapsed/seek/total group
    // keeps it near the centre and lets the band fill the client width, with
    // the volume and search pinned right.
    let bar = TopBar::new(ui, Rect::default())
        .expect("create top bar")
        .icon(PREVIOUS, Glyph::Previous)
        .tooltip(PREVIOUS, "Previous")
        .icon(PLAY_PAUSE, play)
        .tooltip(PLAY_PAUSE, if playing { "Pause" } else { "Play" })
        .icon(STOP, Glyph::Stop)
        .tooltip(STOP, "Stop")
        .icon(NEXT, Glyph::Next)
        .tooltip(NEXT, "Next")
        .toggle(REPEAT, Glyph::Repeat)
        .tooltip(REPEAT, "Repeat")
        .toggle(SHUFFLE, Glyph::Shuffle)
        .tooltip(SHUFFLE, "Shuffle")
        .label(ELAPSED, "0:00")
        .slider(SEEK, 0.0, 1.0)
        // The seek slider absorbs the band's leftover width, so it stretches to
        // fill the space between the transport and the volume/search controls.
        .expand(SEEK)
        .tooltip(SEEK, "Seek")
        .label(TOTAL, "")
        .slider(VOLUME, 0.0, 1.0)
        .tooltip(VOLUME, "Volume")
        .icon(SEARCH, Glyph::Search)
        .tooltip(SEARCH, "Search the library");

    bar.on_click(move |id| {
        if id == PREVIOUS {
            Some(Msg::Dispatch(Command::PlayerPrevious))
        } else if id == PLAY_PAUSE {
            Some(Msg::Dispatch(Command::PlayerPlayPause))
        } else if id == STOP {
            Some(Msg::Dispatch(Command::PlayerStop))
        } else if id == NEXT {
            Some(Msg::Dispatch(Command::PlayerNext))
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
