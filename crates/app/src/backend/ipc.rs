//! Single-instance detection and IPC wiring (#11), built on `winshell`.
//!
//! The frontend-agnostic core (`app_id`, [`IpcBridge`], [`resolve_paths`])
//! lives in `emusic-ui`; only the egui-bound [`RepaintHandle`] stays here
//! (it will become a generic `Waker` in #95).
//!
//! The tricky part is ordering: [`winshell::SingleInstance::acquire`] must
//! happen *before* any window is created, but its `waker` callback (invoked
//! from a background thread whenever a message arrives) needs an
//! [`eframe::egui::Context`] to request a repaint — and that context isn't
//! available until inside `eframe::run_native`'s app-creation closure, which
//! is what creates the window. [`RepaintHandle`] bridges the two: it hands
//! out a waker immediately (a no-op until bound) and is [`RepaintHandle::bind`]-ed
//! to the real context once the window exists.

use std::sync::{Arc, Mutex};

use eframe::egui;

pub use emusic_ui::backend::ipc::{IpcBridge, app_id, resolve_paths};

/// A waker that can be created before a window exists and bound to a real
/// [`egui::Context`] once one does.
#[derive(Clone, Default)]
pub struct RepaintHandle {
    ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl RepaintHandle {
    /// Creates an unbound handle (its waker is a no-op until [`Self::bind`]).
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
