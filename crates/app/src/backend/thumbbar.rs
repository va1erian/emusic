//! Windows taskbar thumbnail-toolbar transport buttons (#42), built on
//! [`winshell::thumbbar`].
//!
//! Adds previous / play-pause / next to the taskbar thumbnail preview and
//! folds button presses into the shell's existing [`Command`]s, exactly like
//! the SMTC integration (#26, [`crate::backend::smtc`]) reuses the same queue
//! instead of repeating the transport logic. The play-pause button's glyph
//! and tooltip follow the player's state.
//!
//! The buttons are only added once the shell announces the taskbar button via
//! `TaskbarButtonCreated` (see [`winshell::thumbbar::take_buttons_requested`],
//! filled by the message hook in `main.rs`): adding them at startup silently
//! does nothing. The same announcement re-arrives after an `explorer.exe`
//! restart, at which point the buttons are added again.
//!
//! Like SMTC, creation degrades to a no-op with a logged warning when the
//! shell object cannot be built (no window yet, Explorer not running, ...),
//! so playback never depends on the taskbar integration.

use std::ffi::c_void;

use tracing::{info, warn};

use crate::player_api::{PlaybackStatus, PlayerApi};
use crate::state::{AppState, Command};

use winshell::thumbbar::{self, ThumbBarButton};

/// Taskbar thumbnail-toolbar integration. `inner` is `None` when the shell
/// object could not be created; every method then no-ops.
pub struct ThumbBar {
    inner: Option<winshell::thumbbar::ThumbBar>,
    /// Last play/pause state pushed to the button, so redundant shell
    /// updates are skipped.
    playing: bool,
}

impl ThumbBar {
    /// Creates the shell object for `hwnd`; the buttons appear later, when
    /// the shell announces the taskbar button (see [`ThumbBar::sync`]).
    ///
    /// A missing window handle disables the integration (logged) rather than
    /// panicking, mirroring [`crate::backend::smtc::Smtc::new`].
    pub fn new(hwnd: Option<*mut c_void>) -> Self {
        let Some(hwnd) = hwnd else {
            return Self::disabled("no window handle");
        };
        match winshell::thumbbar::ThumbBar::new(hwnd as isize) {
            Ok(inner) => Self {
                inner: Some(inner),
                playing: false,
            },
            Err(err) => Self::disabled(&err.to_string()),
        }
    }

    /// Disabled integration: no shell object, all methods no-op.
    fn disabled(reason: &str) -> Self {
        warn!(reason, "taskbar thumbnail toolbar unavailable");
        Self {
            inner: None,
            playing: false,
        }
    }

    /// Runs once per frame: adds the buttons when the shell announces the
    /// taskbar button, turns queued button presses into [`Command`]s, then
    /// mirrors the player's play/pause state onto the button.
    pub fn sync(&mut self, player: &dyn PlayerApi, state: &mut AppState) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        if thumbbar::take_buttons_requested() {
            match inner.add_buttons() {
                Ok(()) => {
                    // `add_buttons` shows the play glyph, so the mirrored
                    // state restarts as not-playing; the check below re-pushes
                    // the real state if the player is already playing.
                    self.playing = false;
                    info!("taskbar thumbnail toolbar buttons added");
                }
                Err(err) => warn!(%err, "could not add taskbar thumbnail toolbar buttons"),
            }
        }
        for button in thumbbar::take_clicks() {
            state.push(transport_command(button));
        }
        let playing = player.status() == PlaybackStatus::Playing;
        if playing != self.playing {
            if let Err(err) = inner.set_playing(playing) {
                warn!(%err, "could not update taskbar play/pause button");
            }
            self.playing = playing;
        }
    }
}

/// Maps one taskbar button to the shell command that implements it.
fn transport_command(button: ThumbBarButton) -> Command {
    match button {
        ThumbBarButton::Previous => Command::PlayerPrevious,
        ThumbBarButton::PlayPause => Command::PlayerPlayPause,
        ThumbBarButton::Next => Command::PlayerNext,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_map_to_the_shared_transport_commands() {
        assert_eq!(
            transport_command(ThumbBarButton::Previous),
            Command::PlayerPrevious
        );
        assert_eq!(
            transport_command(ThumbBarButton::PlayPause),
            Command::PlayerPlayPause
        );
        assert_eq!(transport_command(ThumbBarButton::Next), Command::PlayerNext);
    }
}
