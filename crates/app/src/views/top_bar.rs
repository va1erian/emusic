//! Win32 top transport bar (#108): the shared `TopBar` model rendered on
//! xui's top band (`MaterialTopBar`), with a native `Edit` search box in a
//! `Native` slot.
//!
//! All state, formatting and transport intents come from
//! [`emusic_ui::panels::top_bar::TopBar`]; this view only draws it and maps the
//! band's events back onto that model (see [`to_message`]).

use std::time::Duration;

use emusic_ui::panels::top_bar::{TopBar, TopBarMsg};
use emusic_ui::player_api::PlayerApi;
use emusic_ui::state::VisualizerMode;
use xui::prelude::*;

use crate::app::Msg;
use crate::views::visualizer::{self, VisualizerView};

/// The bar's items, so `set_value`/`set_checked`/`set_enabled` can update them
/// cheaply without rebuilding the list.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BarItem {
    Previous = 1,
    PlayPause,
    Stop,
    Next,
    Repeat,
    Shuffle,
    Elapsed,
    Seek,
    Total,
    Volume,
    Visualizer,
    Search,
    /// The clear button shown while the search query is non-empty.
    ClearSearch,
}

impl BarItem {
    fn id(self) -> TopBarId {
        TopBarId::new(self as u64)
    }
}

/// The band's design height, in device-independent pixels.
const HEIGHT_DIP: f32 = 40.0;
/// The volume slider width.
const VOLUME_WIDTH_DIP: f32 = 90.0;
/// The native search box width.
const SEARCH_WIDTH_DIP: f32 = 200.0;
/// The search slot height: the edit's natural single-line height, so the bar
/// centres it in the band instead of stretching it.
const SEARCH_HEIGHT_DIP: f32 = 20.0;
/// Cue banner for the search box.
const SEARCH_CUE: &str = "Search library... (Ctrl+F)";
/// The clear button's side: the search edit's height, so it lines up with it.
const CLEAR_SIZE_DIP: f32 = SEARCH_HEIGHT_DIP;
/// Segoe Fluent Icons `E894`: the clear ("x") glyph, shown in the search box's
/// clear button (xui's `Fluent` doesn't define it).
const CLEAR_GLYPH: char = '\u{E894}';

/// What the item list depends on beyond the per-frame values; the list is only
/// rebuilt when one of these changes.
#[derive(PartialEq)]
struct Signature {
    playing: bool,
    repeat: bool,
    shuffle: bool,
    seek_supported: bool,
    has_duration: bool,
    visualizer: bool,
}

/// The Win32 transport bar and its app-owned search box.
pub struct TopBarView {
    bar: MaterialTopBar<Msg>,
    /// Owned here; the bar moves and resizes it with its slot.
    search: Edit<Msg>,
    /// The spectrum / oscilloscope strip left of the search box.
    visualizer: VisualizerView,
    signature: Option<Signature>,
}

impl TopBarView {
    /// Builds the bar and the search box, and maps the band's events to [`Msg`].
    ///
    /// Returns an error on a window without an extended title bar, or when
    /// DirectWrite is unavailable, so the app can run without the bar.
    pub fn new(ui: &mut Ui<Msg>) -> xui::Result<Self> {
        let bar = MaterialTopBar::new(ui)?;
        let visualizer = VisualizerView::new(ui)?;
        bar.set_height(dip(HEIGHT_DIP));
        let search = Edit::single_line(ui)?
            .cue(SEARCH_CUE)
            .on_change(|text| Some(Msg::TopBarSearch(text.to_owned())));

        let bar = bar.on_event(|event| Some(Msg::TopBar(event)));
        Ok(Self {
            bar,
            search,
            visualizer,
            signature: None,
        })
    }

    /// Pushes the model into the band and mirrors the search query.
    pub fn sync(&mut self, top_bar: &TopBar, search_query: &str, visualizer: bool) {
        let signature = Signature {
            playing: top_bar.is_playing(),
            repeat: top_bar.repeat_active(),
            shuffle: top_bar.shuffle(),
            seek_supported: top_bar.seek_supported(),
            has_duration: top_bar.duration_secs().is_some(),
            visualizer,
        };
        if self.signature.as_ref() != Some(&signature) {
            self.visualizer.set_visible(visualizer);
            self.bar
                .set_items(items(top_bar, &signature, &self.search, &self.visualizer));
            self.signature = Some(signature);
        }

        // Cheap per-frame updates (no list rebuild).
        self.bar
            .set_value(BarItem::Seek.id(), top_bar.position_secs());
        self.bar
            .set_text(BarItem::Elapsed.id(), &top_bar.elapsed_text());
        self.bar.set_text(
            BarItem::Total.id(),
            top_bar.total_text().as_deref().unwrap_or(""),
        );
        self.bar
            .set_value(BarItem::Volume.id(), f64::from(top_bar.volume()));
        self.bar
            .set_checked(BarItem::Repeat.id(), top_bar.repeat_active());
        self.bar
            .set_checked(BarItem::Shuffle.id(), top_bar.shuffle());
        self.bar
            .set_enabled(BarItem::Seek.id(), top_bar.seek_supported());

        // Mirror external query changes (e.g. GoToArtist) without fighting the
        // user's typing.
        if self.search.text() != search_query {
            self.search.set_text(search_query);
        }
    }

    /// Focuses the search box and selects its text, for the Ctrl+F shortcut.
    pub fn focus_search(&self) {
        self.search.focus();
        self.search.select_all();
    }
}

/// Builds the item list for the current structural state.
fn items(
    top_bar: &TopBar,
    signature: &Signature,
    search: &Edit<Msg>,
    visualizer: &VisualizerView,
) -> Vec<TopBarItem> {
    let (play_glyph, play_tip) = if signature.playing {
        (Fluent::PAUSE, "Pause")
    } else {
        (Fluent::PLAY, "Play")
    };
    // Without a known duration a nominal range keeps the (disabled) thumb from
    // collapsing; the position is still shown by the elapsed label.
    let position = top_bar.position_secs();
    let range_end = top_bar
        .duration_secs()
        .unwrap_or(position.max(1.0))
        .max(0.001);

    let mut items = vec![
        TopBarItem::icon_button(BarItem::Previous.id(), Fluent::PREVIOUS).tooltip("Previous"),
        TopBarItem::icon_button(BarItem::PlayPause.id(), play_glyph).tooltip(play_tip),
        TopBarItem::icon_button(BarItem::Stop.id(), Fluent::STOP).tooltip("Stop"),
        TopBarItem::icon_button(BarItem::Next.id(), Fluent::NEXT).tooltip("Next"),
        TopBarItem::spacer(),
        TopBarItem::toggle(BarItem::Repeat.id(), Fluent::REPEAT)
            .tooltip("Repeat")
            .checked(signature.repeat),
        TopBarItem::spacer(),
        TopBarItem::toggle(BarItem::Shuffle.id(), Fluent::SHUFFLE)
            .tooltip("Shuffle")
            .checked(signature.shuffle),
        TopBarItem::spacer(),
        TopBarItem::label(BarItem::Elapsed.id(), top_bar.elapsed_text()),
        TopBarItem::slider(BarItem::Seek.id(), position, 0.0..=range_end)
            .expand(true)
            .enabled(signature.seek_supported)
            .tooltip(if signature.seek_supported {
                "Seek"
            } else {
                "Seeking isn't available for this track"
            }),
        TopBarItem::label(
            BarItem::Total.id(),
            top_bar.total_text().unwrap_or_default(),
        ),
        TopBarItem::spacer(),
        TopBarItem::slider(BarItem::Volume.id(), f64::from(top_bar.volume()), 0.0..=1.0)
            .width(dip(VOLUME_WIDTH_DIP)),
        TopBarItem::spacer(),
    ];
    if signature.visualizer {
        items.push(
            TopBarItem::native(BarItem::Visualizer.id(), dip(visualizer::WIDTH))
                .height(dip(visualizer::HEIGHT))
                .child(visualizer)
                .tooltip("Visualizer (click to change mode)"),
        );
        items.push(TopBarItem::spacer());
    }
    items.push(
        TopBarItem::native(BarItem::Search.id(), dip(SEARCH_WIDTH_DIP))
            .height(dip(SEARCH_HEIGHT_DIP))
            .child(search)
            .tooltip("Search library"),
    );
    // Always present, so appearing/disappearing never shifts the layout; it
    // is the edit's height, with a proportionally smaller glyph.
    items.push(
        TopBarItem::icon_button(BarItem::ClearSearch.id(), CLEAR_GLYPH)
            .width(dip(CLEAR_SIZE_DIP))
            .height(dip(CLEAR_SIZE_DIP))
            .tooltip("Clear search"),
    );
    items
}

impl TopBarView {
    /// Feeds the visualizer strip for this frame (see [`VisualizerView::feed`]).
    pub fn feed_visualizer(&self, mode: VisualizerMode, player: &dyn PlayerApi) {
        self.visualizer.feed(mode, player);
    }
}

/// Maps a band event onto the shared model's intent, or `None` when the item
/// carries no action.
pub fn to_message(event: TopBarEvent) -> Option<TopBarMsg> {
    let id = |item: BarItem| item.id();
    match event {
        TopBarEvent::Click(clicked) => {
            if clicked == id(BarItem::Previous) {
                Some(TopBarMsg::Previous)
            } else if clicked == id(BarItem::PlayPause) {
                Some(TopBarMsg::PlayPause)
            } else if clicked == id(BarItem::Stop) {
                Some(TopBarMsg::Stop)
            } else if clicked == id(BarItem::Next) {
                Some(TopBarMsg::Next)
            } else if clicked == id(BarItem::ClearSearch) {
                // Empties the query; `sync` then clears the native edit to match.
                Some(TopBarMsg::SetSearchQuery(String::new()))
            } else {
                None
            }
        }
        TopBarEvent::Toggle { id: toggled, .. } => {
            if toggled == id(BarItem::Repeat) {
                Some(TopBarMsg::ToggleRepeat)
            } else if toggled == id(BarItem::Shuffle) {
                Some(TopBarMsg::ToggleShuffle)
            } else {
                None
            }
        }
        TopBarEvent::SliderChange { id: slider, value } => {
            if slider == id(BarItem::Seek) {
                Some(TopBarMsg::Seek(Duration::from_secs_f64(value)))
            } else if slider == id(BarItem::Volume) {
                Some(TopBarMsg::SetVolume(value as f32))
            } else {
                None
            }
        }
        // A commit is the same intent; applying it again is harmless (the model
        // guards a non-seekable track).
        TopBarEvent::SliderCommit { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(item: BarItem) -> Option<TopBarMsg> {
        to_message(TopBarEvent::Click(item.id()))
    }

    #[test]
    fn transport_clicks_map_to_their_intents() {
        assert_eq!(click(BarItem::Previous), Some(TopBarMsg::Previous));
        assert_eq!(click(BarItem::PlayPause), Some(TopBarMsg::PlayPause));
        assert_eq!(click(BarItem::Stop), Some(TopBarMsg::Stop));
        assert_eq!(click(BarItem::Next), Some(TopBarMsg::Next));
        assert_eq!(click(BarItem::Repeat), None, "a toggle is not a click");
    }

    #[test]
    fn clear_search_click_empties_the_query() {
        assert_eq!(
            click(BarItem::ClearSearch),
            Some(TopBarMsg::SetSearchQuery(String::new()))
        );
        assert_eq!(click(BarItem::Search), None, "the slot is not a button");
    }

    #[test]
    fn toggles_map_to_repeat_and_shuffle() {
        let toggle = |item: BarItem, checked| {
            to_message(TopBarEvent::Toggle {
                id: item.id(),
                checked,
            })
        };
        assert_eq!(toggle(BarItem::Repeat, true), Some(TopBarMsg::ToggleRepeat));
        assert_eq!(
            toggle(BarItem::Shuffle, true),
            Some(TopBarMsg::ToggleShuffle)
        );
        assert_eq!(toggle(BarItem::Volume, true), None);
    }

    #[test]
    fn slider_changes_map_to_seek_and_volume() {
        let change = |item: BarItem, value| {
            to_message(TopBarEvent::SliderChange {
                id: item.id(),
                value,
            })
        };
        assert_eq!(
            change(BarItem::Seek, 12.0),
            Some(TopBarMsg::Seek(Duration::from_secs(12)))
        );
        assert_eq!(
            change(BarItem::Volume, 0.4),
            Some(TopBarMsg::SetVolume(0.4))
        );
        assert_eq!(change(BarItem::Elapsed, 3.0), None);
    }
}
