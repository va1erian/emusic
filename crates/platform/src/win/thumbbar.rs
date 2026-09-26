#![forbid(unsafe_code)]

//! Windows taskbar thumbnail-toolbar transport buttons, on top of
//! [`winshell::thumbbar`].
//!
//! The buttons are only added once the shell announces the taskbar button via
//! `TaskbarButtonCreated` (observed by the message hook), so adding them at
//! startup would silently do nothing. Presses are drained into portable
//! [`ShellAction`]s; the play-pause glyph follows the player state.

use winshell::thumbbar::{self, ThumbBar, ThumbBarButton};

use crate::shell::{ShellAction, ThumbButton};

/// The taskbar thumbnail-toolbar integration.
pub(crate) struct Thumbbar {
    inner: Option<ThumbBar>,
    /// The button set the app last requested; the Windows shell currently
    /// always gets previous/play-pause/next.
    requested: Vec<ThumbButton>,
    /// The play/pause state last mirrored, so redundant updates are skipped.
    playing: bool,
}

impl Thumbbar {
    /// Creates the shell object for `hwnd`, or a disabled instance when there
    /// is none or the shell refuses it.
    pub(crate) fn new(hwnd: Option<isize>) -> Thumbbar {
        let Some(hwnd) = hwnd else {
            return Thumbbar::disabled("no window handle");
        };
        match ThumbBar::new(hwnd) {
            Ok(inner) => Thumbbar {
                inner: Some(inner),
                requested: Vec::new(),
                playing: false,
            },
            Err(err) => Thumbbar::disabled(&err.to_string()),
        }
    }

    fn disabled(reason: &str) -> Thumbbar {
        tracing::debug!(reason, "taskbar thumbnail toolbar unavailable");
        Thumbbar {
            inner: None,
            requested: Vec::new(),
            playing: false,
        }
    }

    /// Records the transport buttons the app wants shown.
    pub(crate) fn set_buttons(&mut self, buttons: &[ThumbButton]) {
        self.requested = buttons.to_vec();
    }

    /// Adds the buttons once the shell announces the taskbar button, drains
    /// queued presses and mirrors the play/pause glyph.
    pub(crate) fn sync(&mut self, playing: bool) -> Vec<ShellAction> {
        let Some(inner) = self.inner.as_mut() else {
            return Vec::new();
        };
        if thumbbar::take_buttons_requested() {
            match inner.add_buttons() {
                Ok(()) => {
                    // `add_buttons` shows the play glyph; re-push the real
                    // state below.
                    self.playing = false;
                    tracing::info!(buttons = ?self.requested, "taskbar thumbnail toolbar buttons added");
                }
                Err(err) => tracing::debug!(%err, "could not add taskbar thumbnail buttons"),
            }
        }
        let mut actions = Vec::new();
        for button in thumbbar::take_clicks() {
            actions.push(action(button));
        }
        if playing != self.playing {
            if let Err(err) = inner.set_playing(playing) {
                tracing::debug!(%err, "could not update taskbar play/pause button");
            }
            self.playing = playing;
        }
        actions
    }
}

/// Maps one taskbar button to the transport action it stands for.
fn action(button: ThumbBarButton) -> ShellAction {
    match button {
        ThumbBarButton::Previous => ShellAction::Previous,
        ThumbBarButton::PlayPause => ShellAction::PlayPause,
        ThumbBarButton::Next => ShellAction::Next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_map_to_actions() {
        assert_eq!(action(ThumbBarButton::Previous), ShellAction::Previous);
        assert_eq!(action(ThumbBarButton::PlayPause), ShellAction::PlayPause);
        assert_eq!(action(ThumbBarButton::Next), ShellAction::Next);
    }

    #[test]
    fn a_thumbbar_without_a_window_is_a_no_op() {
        let mut thumbbar = Thumbbar::new(None);
        assert!(thumbbar.inner.is_none());
        assert!(thumbbar.sync(false).is_empty());
    }
}
