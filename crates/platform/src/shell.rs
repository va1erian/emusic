#![forbid(unsafe_code)]

//! The portable [`ShellIntegration`] seam and its no-op fallback.
//!
//! An OS shell service (the media overlay, the taskbar progress bar, the
//! thumbnail toolbar) has no shared model across desktops, so xui does not
//! host it. The app instead defines this small trait, implements it per
//! target, and picks one at startup. Nothing here names a Win32 type, so the
//! no-op [`NullShell`] compiles on every target and the Windows implementation
//! is the only code that disappears.

use std::rc::Rc;
use std::time::Duration;

use xui::xui_core::Proxy;

/// The transport buttons an OS shell can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbButton {
    /// Skip to the previous track.
    Previous,
    /// Toggle play/pause.
    PlayPause,
    /// Skip to the next track.
    Next,
}

/// An action the OS shell asked the app to perform.
///
/// The shell hands these back through the [`Proxy`] it was built with, so a
/// button press on a background callback becomes the app's own message queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellAction {
    /// Toggle play/pause.
    PlayPause,
    /// Skip to the next track.
    Next,
    /// Skip to the previous track.
    Previous,
    /// Stop playback.
    Stop,
    /// Seek to an absolute position.
    Seek(Duration),
}

/// The current track, in portable terms, for the OS media overlay/taskbar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NowPlaying {
    /// The track title (empty when nothing is playing).
    pub title: String,
    /// The artist.
    pub artist: String,
    /// The album.
    pub album: String,
    /// Whether playback is running (as opposed to paused).
    pub playing: bool,
    /// The playback position.
    pub position: Duration,
    /// The track length, when known.
    pub duration: Option<Duration>,
    /// The track's filesystem path, so a shell integration can look for cover
    /// art next to it. `None` when nothing is playing.
    pub path: Option<String>,
}

impl NowPlaying {
    /// The playback fraction in `0.0..=1.0`, or `None` when the length is
    /// unknown or zero.
    pub fn fraction(&self) -> Option<f64> {
        let total = self.duration?.as_secs_f64();
        if total <= 0.0 {
            return None;
        }
        Some((self.position.as_secs_f64() / total).clamp(0.0, 1.0))
    }
}

/// An OS service the app can drive: now-playing metadata, playback state,
/// progress, taskbar thumbnail buttons and a badge.
///
/// Implementations are per target; the trait itself is portable, so the app
/// holds a `Box<dyn ShellIntegration>` and never branches on the platform.
pub trait ShellIntegration {
    /// Publishes the current track, or clears the overlay with `None`.
    fn now_playing(&self, meta: Option<&NowPlaying>);

    /// Sets the progress fraction in `0.0..=1.0`, or clears it with `None`.
    fn progress(&self, fraction: Option<f64>);

    /// Sets which transport buttons the taskbar thumbnail shows.
    fn thumb_buttons(&self, buttons: &[ThumbButton]);

    /// Sets a short badge/count label, or clears it with `None`.
    fn set_badge(&self, label: Option<&str>);

    /// Drains OS transport events queued since the last call, posting each
    /// into the app through the [`Proxy`] the shell was built with.
    fn poll(&self);
}

/// A no-op [`ShellIntegration`] for targets with no shell service (and for
/// headless runs, where the app simply does not build one).
#[derive(Clone, Copy, Debug, Default)]
pub struct NullShell;

impl ShellIntegration for NullShell {
    fn now_playing(&self, _meta: Option<&NowPlaying>) {}
    fn progress(&self, _fraction: Option<f64>) {}
    fn thumb_buttons(&self, _buttons: &[ThumbButton]) {}
    fn set_badge(&self, _label: Option<&str>) {}
    fn poll(&self) {}
}

/// Builds the shell integration for this target.
///
/// `handle` is an opaque native-window handle the app obtained from its
/// backend ([`NativeHandle`](crate::NativeHandle)); it is `None` on a backend
/// with no native window (canvas), which yields a [`NullShell`]. `proxy`
/// carries transport actions back into the app; `M` must be constructible
/// from a [`ShellAction`].
pub fn shell<M>(handle: Option<crate::NativeHandle>, proxy: Proxy<M>) -> Box<dyn ShellIntegration>
where
    M: From<ShellAction> + Send + 'static,
{
    #[cfg(windows)]
    {
        crate::win::WinShell::build(handle, proxy)
    }
    #[cfg(not(windows))]
    {
        let _ = (handle, proxy);
        Box::new(NullShell)
    }
}

/// A shared proxy wrapper so implementations can post actions without naming
/// the message type in the trait.
pub(crate) struct ActionSink<M> {
    proxy: Proxy<M>,
}

impl<M> ActionSink<M>
where
    M: From<ShellAction> + Send + 'static,
{
    pub(crate) fn shared(proxy: Proxy<M>) -> Rc<ActionSink<M>> {
        Rc::new(ActionSink { proxy })
    }

    pub(crate) fn post(&self, action: ShellAction) {
        let _ = self.proxy.send(M::from(action));
    }
}
