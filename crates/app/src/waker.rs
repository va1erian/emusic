//! The frontend's [`Waker`]: wakes the Win32 UI from background work (#106).
//!
//! Background workers (the search engines, single-instance IPC, image decodes)
//! hold an [`emusic_ui::waker::WakerHandle`] and, when they have something new,
//! call [`Waker::wake`]. Here that posts [`Msg::Wake`] through xui's
//! thread-safe [`Proxy`], which coalesces a burst into a single posted wake and
//! delivers it to [`Win32App::update`](crate::app::Win32App) on the UI thread.

use emusic_ui::waker::Waker;
use xui::Proxy;

use crate::app::Msg;

/// Wakes the app by posting a message to its window.
pub struct Win32Waker(Proxy<Msg>);

impl Win32Waker {
    /// Wraps the window's proxy, obtained from [`xui::Ui::proxy`].
    #[must_use]
    pub fn new(proxy: Proxy<Msg>) -> Self {
        Self(proxy)
    }
}

impl Waker for Win32Waker {
    fn wake(&self) {
        // A dead window hands the message back; the worker will stop on its
        // next wake, so there is nothing to do about it here.
        let _ = self.0.send(Msg::Wake);
    }
}
