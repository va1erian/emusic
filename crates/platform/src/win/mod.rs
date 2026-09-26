#![forbid(unsafe_code)]

//! The Windows implementation of [`ShellIntegration`](crate::ShellIntegration).
//!
//! Every module here is Windows-only. [`WinShell`] wires the three shell
//! services together (SMTC, taskbar thumbnail buttons, taskbar progress and
//! tooltip) behind the portable trait and forwards their transport events into
//! the app through a `Proxy`.

mod assoc;
mod smtc;
mod taskbar;
mod thumbbar;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::shell::{ActionSink, ShellAction, ShellIntegration, ThumbButton};
use crate::window::NativeHandle;

use smtc::Smtc;
use taskbar::Taskbar;
use thumbbar::Thumbbar;

/// Registers the `TaskbarButtonCreated` message before any window exists, so
/// the thumbnail-toolbar hook recognises the announcement. Idempotent.
pub fn prepare() {
    winshell::thumbbar::taskbar_button_created_message();
}

/// Registers this process as the handler for emusic's file extensions.
///
/// Per-user (no elevation), under `HKCU\Software\Classes`. Errors are returned
/// as text so the app binary can wrap them in `anyhow`.
pub fn register_associations() -> Result<(), String> {
    assoc::register()
}

/// Removes the file associations registered by [`register_associations`].
pub fn unregister_associations() -> Result<(), String> {
    assoc::unregister()
}

/// The Windows shell integration.
pub(crate) struct WinShell<M> {
    sink: Rc<ActionSink<M>>,
    smtc: RefCell<Smtc>,
    taskbar: RefCell<Taskbar>,
    thumbbar: RefCell<Thumbbar>,
    /// Whether playback is running, mirrored onto the thumbnail button.
    playing: Cell<bool>,
}

impl<M> WinShell<M>
where
    M: From<ShellAction> + Send + 'static,
{
    /// Builds the integration for `handle`, or a disabled one when there is no
    /// native window (each service then no-ops).
    pub(crate) fn build(
        handle: Option<NativeHandle>,
        proxy: xui::xui_core::Proxy<M>,
    ) -> Box<dyn ShellIntegration> {
        let raw = handle.map(NativeHandle::raw);
        Box::new(WinShell {
            sink: ActionSink::shared(proxy),
            smtc: RefCell::new(Smtc::new(raw)),
            taskbar: RefCell::new(Taskbar::new(raw)),
            thumbbar: RefCell::new(Thumbbar::new(raw)),
            playing: Cell::new(false),
        })
    }
}

impl<M> ShellIntegration for WinShell<M>
where
    M: From<ShellAction> + Send + 'static,
{
    fn now_playing(&self, meta: Option<&crate::NowPlaying>) {
        self.playing.set(meta.is_some_and(|meta| meta.playing));
        self.smtc.borrow_mut().set_now_playing(meta);
        self.taskbar.borrow_mut().set_now_playing(meta);
    }

    fn progress(&self, fraction: Option<f64>) {
        self.taskbar
            .borrow_mut()
            .set_progress(fraction, self.playing.get());
    }

    fn thumb_buttons(&self, buttons: &[ThumbButton]) {
        self.thumbbar.borrow_mut().set_buttons(buttons);
    }

    fn set_badge(&self, _label: Option<&str>) {
        // The Windows taskbar badge is an `ITaskbarList3` overlay icon. It is
        // not wired yet (follow-up #377); the integration stays a no-op.
    }

    fn poll(&self) {
        for action in self.smtc.borrow_mut().drain(self.playing.get()) {
            self.sink.post(action);
        }
        for action in self.thumbbar.borrow_mut().sync(self.playing.get()) {
            self.sink.post(action);
        }
    }
}
