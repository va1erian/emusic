#![forbid(unsafe_code)]

//! The value `main` resolves once and hands to whichever frontend crate is
//! compiled in.
//!
//! Keeping the session in `emusic-ui` lets the binary and both frontends agree
//! on one type without depending on each other. The binary owns the CLI,
//! file-association and single-instance bootstrap; a frontend only opens its
//! window and runs the shared [`Shell`](crate::shell::Shell).
//!
//! A frontend is any crate exposing `pub fn run(Startup) -> anyhow::Result<()>`
//! — see [`RunFn`]. The binary checks the selected frontend against that
//! signature at compile time, so adding a renderer (a native one, a portable
//! one, a future GTK/Cocoa one) needs no change here.

use std::path::PathBuf;

use crate::backend::ipc::IpcBridge;
use crate::config::Config;
use crate::waker::WakerSlot;

/// The entry-point signature every frontend crate exposes as `run`.
///
/// The binary stores the selected frontend in a `const` of this type, which
/// fails to compile if a frontend's `run` drifts from the contract.
pub type RunFn = fn(Startup) -> anyhow::Result<()>;

/// Everything a frontend needs to open the main window and run the app.
pub struct Startup {
    /// The loaded user configuration.
    pub config: Config,
    /// Where the configuration is persisted, or `None` for a `--mock` run that
    /// must not touch the real user's config.
    pub config_path: Option<PathBuf>,
    /// The IPC bridge when this process is the primary instance (Windows only
    /// for now; `None` elsewhere and for screenshot tools).
    pub ipc: Option<IpcBridge>,
    /// Files requested on the command line (or forwarded by a secondary
    /// launch) to enqueue at startup.
    pub files: Vec<PathBuf>,
    /// Whether forwarded files should be enqueued rather than played.
    pub enqueue: bool,
    /// The UI-side end of the worker wake seam.
    pub waker: WakerSlot,
    /// Whether this is a deterministic `--mock` run.
    pub mock: bool,
}
