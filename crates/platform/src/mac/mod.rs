#![forbid(unsafe_code)]

//! The macOS implementation of [`ShellIntegration`](crate::ShellIntegration).
//!
//! macOS has no taskbar thumbnail toolbar, taskbar progress bar or overlay
//! badge, so the only OS service is the Now Playing info centre and the
//! hardware media keys, driven through `souvlaki` by [`NowPlaying`].

mod now_playing;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::shell::{ActionSink, ShellAction, ShellIntegration, ThumbButton};
use crate::window::NativeHandle;

use now_playing::NowPlaying;

/// The macOS shell integration.
pub(crate) struct MacShell<M> {
    sink: Rc<ActionSink<M>>,
    now_playing: RefCell<NowPlaying>,
    /// Whether playback is running, for the transport events' context.
    playing: Cell<bool>,
}

impl<M> MacShell<M>
where
    M: From<ShellAction> + Send + 'static,
{
    /// Builds the integration. `handle` is unused: the macOS media controls
    /// need no window handle.
    pub(crate) fn build(
        _handle: Option<NativeHandle>,
        proxy: xui::xui_core::Proxy<M>,
    ) -> Box<dyn ShellIntegration> {
        Box::new(MacShell {
            sink: ActionSink::shared(proxy),
            now_playing: RefCell::new(NowPlaying::new()),
            playing: Cell::new(false),
        })
    }
}

impl<M> ShellIntegration for MacShell<M>
where
    M: From<ShellAction> + Send + 'static,
{
    fn now_playing(&self, meta: Option<&crate::NowPlaying>) {
        self.playing.set(meta.is_some_and(|meta| meta.playing));
        self.now_playing.borrow_mut().set_now_playing(meta);
    }

    fn progress(&self, _fraction: Option<f64>) {
        // The Now Playing overlay carries the position with the metadata;
        // macOS has no separate taskbar progress bar.
    }

    fn thumb_buttons(&self, _buttons: &[ThumbButton]) {
        // macOS has no taskbar thumbnail toolbar.
    }

    fn set_badge(&self, _label: Option<&str>) {
        // macOS has no taskbar overlay badge.
    }

    fn poll(&self) {
        for action in self.now_playing.borrow_mut().drain(self.playing.get()) {
            self.sink.post(action);
        }
    }
}
