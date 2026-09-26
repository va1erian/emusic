//! The frontend's [`Waker`]: wakes the UI from background work (#106).
//!
//! Background workers (the search engines, single-instance IPC, image decodes)
//! hold an [`emusic_ui::waker::WakerHandle`] and, when they have something new,
//! call [`Waker::wake`]. Here that posts [`Msg::Wake`] through the portable
//! [`Proxy`], which carries a worker's message to the UI thread and coalesces a
//! burst into a single drain.

use emusic_ui::waker::Waker;
use xui::xui_core::Proxy;

use crate::app::Msg;

/// Wakes the app by posting a message to its window.
pub struct UiWaker(Proxy<Msg>);

impl UiWaker {
    /// Wraps the window's proxy, obtained from [`Ui::proxy`](xui::xui_core::Ui::proxy).
    #[must_use]
    pub fn new(proxy: Proxy<Msg>) -> Self {
        Self(proxy)
    }
}

impl Waker for UiWaker {
    fn wake(&self) {
        // A dead window hands the message back; the worker stops on its next
        // wake, so there is nothing to do about it here.
        let _ = self.0.send(Msg::Wake);
    }
}
