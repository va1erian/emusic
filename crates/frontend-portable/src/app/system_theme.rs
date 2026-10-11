//! Following the host's colour scheme and accent (LazyOS: the desktop's).
//!
//! Settings → Appearance offers "Follow system" when the host's backend has a
//! theme to give ([`Backend::system_theme`]); until the user picks a scheme by
//! hand it is on. The host's theme is read again about once a second so the
//! window re-themes when the desktop does, without the host having to push.

use std::rc::Rc;
use std::time::{Duration, Instant};

use emusic_ui::state::Theme;
use xui::xui_core::backend::Backend;
use xui::xui_core::theme::Theme as UiTheme;

use super::Win32App;
use crate::theme::Look;

/// How often the host's theme is read again.
const POLL: Duration = Duration::from_secs(1);

/// The host's theme source and the last theme it gave.
#[derive(Default)]
pub(super) struct SystemTheme {
    source: Option<Rc<dyn Backend>>,
    latest: Option<UiTheme>,
    polled: Option<Instant>,
}

impl SystemTheme {
    /// Reads the host's theme if it is due (or `force`d); returns whether it
    /// changed.
    fn poll(&mut self, force: bool, now: Instant) -> bool {
        let Some(source) = &self.source else {
            return false;
        };
        if !force && self.polled.is_some_and(|at| now.duration_since(at) < POLL) {
            return false;
        }
        self.polled = Some(now);
        let theme = source.system_theme();
        // A host that stops answering keeps the last theme it gave.
        if theme.is_none() || theme == self.latest {
            return false;
        }
        self.latest = theme;
        true
    }

    /// The repeat interval the shell asked for, shortened so the host's
    /// theme is read while the app is otherwise idle.
    pub(super) fn cap_wake(&self, wake: Option<Duration>) -> Option<Duration> {
        if self.latest.is_none() {
            return wake;
        }
        Some(wake.map_or(POLL, |wake| wake.min(POLL)))
    }
}

impl Win32App {
    /// Lets the window follow `host`'s theme (when it has one), applying it
    /// now. Without a system theme the window keeps its own look.
    pub fn follow_system_theme(&mut self, host: Rc<dyn Backend>) {
        self.system.source = Some(host);
        self.system.poll(true, Instant::now());
        self.sync_look();
    }

    /// Whether the window is painted in the host's theme.
    fn following_system(&self) -> bool {
        self.system.latest.is_some() && self.shell.state.follow_system.unwrap_or(true)
    }

    /// The look the window should have now, reading the host's theme if due.
    pub(super) fn current_look(&mut self) -> Look {
        self.system.poll(false, Instant::now());
        match self.system.latest {
            Some(theme) if self.following_system() => {
                // Keep the stored scheme in step, so the View menu's toggle
                // goes to the other one and a later opt-out starts from it.
                self.shell.state.theme = if theme.is_dark {
                    Theme::Dark
                } else {
                    Theme::Light
                };
                Look::System(theme)
            }
            _ => Look::Own(self.shell.state.theme, self.shell.state.accent),
        }
    }

    /// Applies [`Self::current_look`] if it differs from the applied one.
    fn sync_look(&mut self) {
        let look = self.current_look();
        if look != self.applied_look {
            self.applied_look = look;
            self.ui.set_theme(look.resolve());
            self.navigator.set_dark(self.ui.theme().is_dark);
        }
    }
}
