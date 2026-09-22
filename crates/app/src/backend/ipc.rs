//! Single-instance detection and IPC wiring (#11), built on `winshell`.
//!
//! The tricky part is ordering: [`winshell::SingleInstance::acquire`] must
//! happen *before* any window is created, but its `waker` callback (invoked
//! from a background thread whenever a message arrives) needs an
//! [`eframe::egui::Context`] to request a repaint — and that context isn't
//! available until inside `eframe::run_native`'s app-creation closure, which
//! is what creates the window. [`RepaintHandle`] bridges the two: it hands
//! out a waker immediately (a no-op until bound) and is [`RepaintHandle::bind`]-ed
//! to the real context once the window exists.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui;

/// A waker that can be created before a window exists and bound to a real
/// [`egui::Context`] once one does.
#[derive(Clone, Default)]
pub struct RepaintHandle {
    ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl RepaintHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// A `Fn() + Send + Sync` closure suitable for
    /// [`winshell::SingleInstance::acquire`]; harmless to call before
    /// [`Self::bind`].
    pub fn waker(&self) -> impl Fn() + Send + Sync + 'static {
        let ctx = Arc::clone(&self.ctx);
        move || {
            if let Some(ctx) = ctx.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                ctx.request_repaint();
            }
        }
    }

    /// Connects this handle to the real context, once the window exists.
    pub fn bind(&self, ctx: egui::Context) {
        *self.ctx.lock().unwrap_or_else(|e| e.into_inner()) = Some(ctx);
    }
}

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
/// the primary. A secondary process never gets this far (see `cli.rs`'s
/// startup flow) so this only ever wraps `Some` in that case; the `None`
/// path exists for `emusic-shot`/tests, which never do IPC at all.
#[derive(Default)]
pub struct IpcBridge {
    listener: Option<winshell::Listener>,
}

impl IpcBridge {
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
