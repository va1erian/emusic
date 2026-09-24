//! Single-instance detection and IPC wiring (#11), built on `winshell`.
//!
//! Moved from the egui frontend (#93). The egui-bound [`RepaintHandle`] that
//! used to live here stays in the frontend (see #95); everything here works
//! with any `Fn()` waker.

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

/// Holds the primary instance's [`winshell::Listener`], if this process is
/// the primary. A secondary process never gets this far (see the CLI startup
/// flow) so this only ever wraps `Some` in that case; the `None` path exists
/// for screenshot tools/tests, which never do IPC at all.
#[derive(Default)]
pub struct IpcBridge {
    listener: Option<winshell::Listener>,
}

impl IpcBridge {
    /// Wraps the primary instance's listener.
    pub fn primary(listener: winshell::Listener) -> Self {
        Self {
            listener: Some(listener),
        }
    }

    /// Returns the next batched message forwarded by a secondary launch, if
    /// one has arrived since the last poll.
    pub fn try_recv(&self) -> Option<winshell::IpcMessage> {
        self.listener
            .as_ref()
            .and_then(winshell::Listener::try_recv)
    }
}

/// Resolves an [`winshell::IpcMessage`]'s (possibly relative) file paths
/// against the working directory the sender recorded them from.
pub fn resolve_paths(message: &winshell::IpcMessage) -> Vec<PathBuf> {
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
