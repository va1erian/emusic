#![forbid(unsafe_code)]

//! Windows System Media Transport Controls (SMTC) through `souvlaki`.
//!
//! A thin platform layer over the shared [`Media`](crate::media::Media)
//! wrapper: it only supplies the Windows [`PlatformConfig`] (the window
//! handle).

use std::ffi::c_void;

use souvlaki::PlatformConfig;

use crate::media::Media;
use crate::shell::{NowPlaying, ShellAction};

/// The Windows SMTC integration and its transport events.
pub(crate) struct Smtc {
    media: Media,
}

impl Smtc {
    /// Creates the media controls bound to `hwnd`, or a disabled instance when
    /// there is no window or the platform call fails.
    pub(crate) fn new(hwnd: Option<isize>) -> Smtc {
        let config = hwnd.map(|hwnd| PlatformConfig {
            display_name: "emusic",
            dbus_name: "emusic",
            hwnd: Some(hwnd as *mut c_void),
        });
        Smtc {
            media: Media::new(config, "SMTC"),
        }
    }

    /// Publishes `meta` to the overlay. `None` clears it.
    pub(crate) fn set_now_playing(&mut self, meta: Option<&NowPlaying>) {
        self.media.set_now_playing(meta);
    }

    /// Drains the OS transport events, mapping each to a portable action.
    pub(crate) fn drain(&mut self, playing: bool) -> Vec<ShellAction> {
        self.media.drain(playing)
    }
}
