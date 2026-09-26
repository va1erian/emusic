//! Single instance detection and IPC between emusic processes.
//!
//! The very first process for a given `app_id` becomes the *primary*: it
//! owns the named pipe `\\.\pipe\<app_id>` outright (Windows refuses a
//! second "first instance" of the same pipe name), so no separate mutex is
//! needed to detect the race. Every later launch is a *secondary*: it fails
//! to become primary, forwards its command-line files to the primary over
//! that same pipe, and exits.

mod message;
#[cfg(windows)]
mod primary;
#[cfg(windows)]
mod secondary;

use std::io;

pub use message::IpcMessage;
#[cfg(windows)]
pub use primary::Listener;
#[cfg(windows)]
pub use secondary::send_to_primary;

#[cfg(not(windows))]
#[derive(Debug)]
pub struct Listener;

#[cfg(not(windows))]
impl Listener {
    pub fn take_messages(&self) -> Vec<IpcMessage> {
        Vec::new()
    }
}

#[cfg(not(windows))]
pub fn send_to_primary(_app_id: &str, _message: &IpcMessage) -> crate::Result<()> {
    Ok(())
}

/// The result of [`SingleInstance::acquire`].
pub enum SingleInstance {
    /// This process is the primary instance.
    Primary(Listener),
    /// Another process is already primary.
    Secondary,
}

impl SingleInstance {
    #[cfg(windows)]
    pub fn acquire(app_id: &str, waker: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        match Listener::start(app_id, waker) {
            Ok(listener) => Ok(Self::Primary(listener)),
            Err(e) if e.raw_os_error() == Some(primary::ERROR_ACCESS_DENIED) => Ok(Self::Secondary),
            Err(e) => Err(e),
        }
    }

    #[cfg(not(windows))]
    pub fn acquire(_app_id: &str, _waker: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        Ok(Self::Primary(Listener))
    }
}

/// The full named-pipe path for `app_id`.
fn pipe_path(app_id: &str) -> String {
    format!(r"\\.\pipe\{app_id}")
}
