#![forbid(unsafe_code)]

//! The macOS Now Playing info centre and hardware media keys, through the
//! shared [`Media`](crate::media::Media) wrapper.

use souvlaki::PlatformConfig;

use crate::media::Media;
use crate::shell::{NowPlaying as NowPlayingInfo, ShellAction};

/// The macOS media controls and their transport events.
pub(crate) struct NowPlaying {
    media: Media,
}

impl NowPlaying {
    /// Creates the media controls. macOS ignores the window handle; the
    /// display name is what the Now Playing widget shows.
    pub(crate) fn new() -> NowPlaying {
        let config = PlatformConfig {
            display_name: "emusic",
            dbus_name: "emusic",
            hwnd: None,
        };
        NowPlaying {
            media: Media::new(Some(config), "Now Playing"),
        }
    }

    /// Publishes `meta` to the overlay. `None` clears it.
    pub(crate) fn set_now_playing(&mut self, meta: Option<&NowPlayingInfo>) {
        self.media.set_now_playing(meta);
    }

    /// Drains the OS transport events, mapping each to a portable action.
    pub(crate) fn drain(&mut self, playing: bool) -> Vec<ShellAction> {
        self.media.drain(playing)
    }
}
