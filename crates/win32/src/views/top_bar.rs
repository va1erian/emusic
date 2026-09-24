//! Win32 top transport bar (#108): the shared `TopBar` model rendered on
//! win32ui's top band (`MaterialTopBar`), with a native `Edit` search box in a
//! `Native` slot.
//!
//! All state, formatting and transport intents come from
//! [`emusic_ui::panels::top_bar::TopBar`]; this view only draws it and maps the
//! band's events back onto that model (see [`to_message`]).

use std::time::Duration;

use emusic_ui::panels::top_bar::{TopBar, TopBarMsg};
use win32ui::prelude::*;

use crate::app::Msg;

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
    Search,
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
/// Cue banner for the search box, matching the egui top bar.
const SEARCH_CUE: &str = "Search library... (Ctrl+F)";

/// What the item list depends on beyond the per-frame values; the list is only
/// rebuilt when one of these changes.
#[derive(PartialEq)]
struct Signature {
    playing: bool,
    repeat: bool,
    shuffle: bool,
    seek_supported: bool,
    has_duration: bool,
}

/// The Win32 transport bar and its app-owned search box.
pub struct TopBarView {
    bar: MaterialTopBar<Msg>,
    search: Edit<Msg>,
    signature: Option<Signature>,
}

impl TopBarView {
    /// Builds the bar and the search box, and maps the band's events to [`Msg`].
    ///
    /// Returns an error on a window without an extended title bar, or when
    /// DirectWrite is unavailable, so the app can run without the bar.
    pub fn new(ui: &mut Ui<Msg>) -> win32ui::Result<Self> {
        let bar = MaterialTopBar::new(ui)?;
        bar.set_height(dip(HEIGHT_DIP));
        let search = Edit::single_line(ui)?
            .cue(SEARCH_CUE)
            .on_change(|text| Some(Msg::TopBarSearch(text.to_owned())));
        let bar = bar.on_event(|event| Some(Msg::TopBar(event)));
        Ok(Self {
            bar,
            search,
            signature: None,
        })
    }

    /// Pushes the model into the band and keeps the search box in its slot.
    pub fn sync(&mut self, ui: &Ui<Msg>, top_bar: &TopBar, search_query: &str) {
        let signature = Signature {
            playing: top_bar.is_playing(),
            repeat: top_bar.repeat_active(),
            shuffle: top_bar.shuffle(),
            seek_supported: top_bar.seek_supported(),
            has_duration: top_bar.duration_secs().is_some(),
        };
        if self.signature.as_ref() != Some(&signature) {
            self.bar.set_items(items(top_bar, &signature));
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

        // Keep the native search box in its slot, and mirror external query
        // changes (e.g. GoToArtist) without fighting the user's typing.
        if let Some(rect) = ui.material_top_bar_slot(BarItem::Search.id()) {
            self.search.set_bounds(rect);
        }
        if self.search.text() != search_query {
            self.search.set_text(search_query);
        }
    }
}

/// Builds the item list for the current structural state.
fn items(top_bar: &TopBar, signature: &Signature) -> Vec<TopBarItem> {
    let (play_glyph, play_tip) = if signature.playing {
        (Fluent::PAUSE, "Pause")
    } else {
        (Fluent::PLAY, "Play")
    };
    // Without a known duration a nominal range keeps the (disabled) thumb from
    // collapsing; the position is still shown by the elapsed label, matching
    // the egui top bar.
    let position = top_bar.position_secs();
    let range_end = top_bar
        .duration_secs()
        .unwrap_or(position.max(1.0))
        .max(0.001);

    vec![
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
        TopBarItem::native(BarItem::Search.id(), dip(SEARCH_WIDTH_DIP)).tooltip("Search library"),
    ]
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
