//! Single instance detection and IPC between emusic processes.
//!
//! The very first process for a given `app_id` becomes the *primary*: it
//! owns the named pipe `\\.\pipe\<app_id>` outright (Windows refuses a
//! second "first instance" of the same pipe name), so no separate mutex is
//! needed to detect the race. Every later launch is a *secondary*: it fails
//! to become primary, forwards its command-line files to the primary over
//! that same pipe, and exits.

mod message;
mod primary;
mod secondary;

use std::io;

pub use message::IpcMessage;
pub use primary::Listener;
pub use secondary::send_to_primary;

use primary::ERROR_ACCESS_DENIED;

/// The result of [`SingleInstance::acquire`].
pub enum SingleInstance {
    /// This process is the primary instance; `Listener` receives requests
    /// forwarded by later, secondary launches.
    Primary(Listener),
    /// Another process is already primary. The caller should forward its
    /// own arguments with [`send_to_primary`] and exit.
    Secondary,
}

impl SingleInstance {
    /// Attempts to become the primary instance for `app_id` (e.g.
    /// `"emusic-<user sid>"`, so different users on the same machine don't
    /// collide).
    ///
    /// `waker` is called from the listener's background thread every time a
    /// new batch of messages is ready, so the caller can e.g.
    /// `ctx.request_repaint()`; it is never called if this becomes a
    /// [`SingleInstance::Secondary`].
    pub fn acquire(app_id: &str, waker: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        match Listener::start(app_id, waker) {
            Ok(listener) => Ok(Self::Primary(listener)),
            Err(e) if e.raw_os_error() == Some(ERROR_ACCESS_DENIED) => Ok(Self::Secondary),
            Err(e) => Err(e),
        }
    }
}

/// The full named-pipe path for `app_id`.
fn pipe_path(app_id: &str) -> String {
    format!(r"\\.\pipe\{app_id}")
}
