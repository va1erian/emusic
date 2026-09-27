//! Single-instance detection and IPC wiring (#11).
//!
//! On Windows this rides on `winshell`'s named-pipe handshake. Everywhere else
//! there is no single-instance/IPC transport yet (#377 adds a lock-file
//! equivalent), so [`IpcBridge`] is inert and never yields a message. The
//! [`IpcMessage`] type is portable so the controller's handling stays
//! platform-agnostic.
//!
//! The repaint handle that used to live here is now the toolkit-agnostic
//! [`Waker`] seam (#95).
//!
//! [`Waker`]: crate::waker::Waker

use std::path::PathBuf;

/// Per-user, per-mode identifier for the named pipe `winshell` uses to
/// detect a running instance. Kept distinct for `--mock` so a development
/// session never fights with (or forwards files into) a real one.
pub fn app_id(mock: bool) -> String {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
    if mock {
        format!("emusic-mock-{user}")
    } else {
        format!("emusic-{user}")
    }
}

/// A batch of work a secondary launch forwards to the primary instance (#11):
/// the files to open and whether they should be enqueued instead of replacing
/// the queue. Portable so [`crate::shell::Shell::handle_ipc_message`] needs no
/// `cfg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcMessage {
    /// Append the files to the queue instead of replacing it.
    pub enqueue: bool,
    /// The files the secondary launch was asked to open.
    pub files: Vec<PathBuf>,
    /// The working directory the sender recorded them from, for resolving
    /// relative paths.
    pub cwd: PathBuf,
}

/// Holds the primary instance's listener, if this process is the primary. A
/// secondary process never gets this far (see the CLI startup flow) so this
/// only ever wraps `Some` in that case; the `None`/default path exists for
/// non-Windows builds, screenshot tools and tests, which never do IPC at all.
#[derive(Default)]
pub struct IpcBridge {
    #[cfg(windows)]
    listener: Option<winshell::Listener>,
}

impl IpcBridge {
    /// Wraps the primary instance's listener. Windows-only.
    #[cfg(windows)]
    pub fn primary(listener: winshell::Listener) -> Self {
        Self {
            listener: Some(listener),
        }
    }

    /// Returns the next batched message forwarded by a secondary launch, if
    /// one has arrived since the last poll. Always `None` off Windows.
    pub fn try_recv(&self) -> Option<IpcMessage> {
        #[cfg(windows)]
        {
            self.listener
                .as_ref()
                .and_then(winshell::Listener::try_recv)
                .map(IpcMessage::from)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
}

#[cfg(windows)]
impl From<winshell::IpcMessage> for IpcMessage {
    fn from(message: winshell::IpcMessage) -> Self {
        Self {
            enqueue: message.enqueue,
            files: message.files,
            cwd: message.cwd,
        }
    }
}

/// Resolves an [`IpcMessage`]'s (possibly relative) file paths against the
/// working directory the sender recorded them from.
pub fn resolve_paths(message: &IpcMessage) -> Vec<PathBuf> {
    message
        .files
        .iter()
        .map(|f| {
            if f.is_absolute() {
                f.clone()
            } else {
                message.cwd.join(f)
            }
        })
        .collect()
}
