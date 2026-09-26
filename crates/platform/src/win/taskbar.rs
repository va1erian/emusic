#![forbid(unsafe_code)]

//! Windows taskbar progress bar and button tooltip, on top of
//! [`winshell::taskbar`].
//!
//! Mirrors the current track's `title - artist` into the button tooltip and
//! the playback fraction into the taskbar progress bar. The DWM iconic
//! thumbnail card is not wired yet (it needs the app's artwork cache; tracked
//! in follow-up #377), so the card falls back to the shell's default preview.

use winshell::taskbar::{ProgressState, TaskBar};

use crate::shell::NowPlaying;

/// The taskbar progress-bar and tooltip integration.
pub(crate) struct Taskbar {
    inner: Option<TaskBar>,
    /// Whether playback is running, so a determinate bar can show paused.
    playing: bool,
    /// Whether the "unavailable" warning was already logged.
    warned: bool,
}

impl Taskbar {
    /// Creates the shell object for `hwnd`, or a disabled instance when there
    /// is none or the shell refuses it.
    pub(crate) fn new(hwnd: Option<isize>) -> Taskbar {
        let Some(hwnd) = hwnd else {
            return Taskbar::disabled("no window handle");
        };
        match TaskBar::new(hwnd) {
            Ok(inner) => Taskbar {
                inner: Some(inner),
                playing: false,
                warned: false,
            },
            Err(err) => Taskbar::disabled(&err.to_string()),
        }
    }

    fn disabled(reason: &str) -> Taskbar {
        tracing::debug!(reason, "taskbar progress bar unavailable");
        Taskbar {
            inner: None,
            playing: false,
            warned: false,
        }
    }

    /// Sets the button tooltip from the current track and records the playing
    /// state used by [`Taskbar::set_progress`].
    pub(crate) fn set_now_playing(&mut self, meta: Option<&NowPlaying>) {
        self.playing = meta.is_some_and(|meta| meta.playing);
        let tooltip = match meta {
            Some(meta) if !meta.title.is_empty() && !meta.artist.is_empty() => {
                format!("{} - {}", meta.title, meta.artist)
            }
            Some(meta) if !meta.title.is_empty() => meta.title.clone(),
            _ => "emusic".to_string(),
        };
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        if let Err(err) = inner.set_tooltip(&tooltip) {
            self.warn_once(&err, "could not set the taskbar tooltip");
        }
    }

    /// Sets the progress bar from `fraction` (`0.0..=1.0`), or clears it with
    /// `None`.
    pub(crate) fn set_progress(&mut self, fraction: Option<f64>, playing: bool) {
        let Some(inner) = self.inner.as_mut() else {
            return;
        };
        let state = if fraction.is_some() {
            if playing {
                ProgressState::Normal
            } else {
                ProgressState::Paused
            }
        } else {
            ProgressState::None
        };
        let (completed, total) = fraction
            .map(|fraction| ((fraction.clamp(0.0, 1.0) * 1000.0).round() as u64, 1000))
            .unwrap_or((0, 0));
        if let Err(err) = inner.set_progress(state, completed, total) {
            self.warn_once(&err, "could not update the taskbar progress bar");
        }
    }

    /// Logs a shell failure once, so a per-tick failure cannot flood the log.
    fn warn_once(&mut self, err: &winshell::WinshellError, message: &str) {
        if !self.warned {
            tracing::debug!(%err, message);
            self.warned = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taskbar_without_a_window_is_a_no_op() {
        let mut taskbar = Taskbar::new(None);
        assert!(taskbar.inner.is_none());
        taskbar.set_now_playing(None);
        taskbar.set_progress(Some(0.5), true);
    }
}
