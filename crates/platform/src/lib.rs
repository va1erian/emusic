#![forbid(unsafe_code)]

//! emusic's OS shell integration.
//!
//! This is the one crate that hosts the platform-specific pieces of the app:
//! now-playing metadata on the OS media overlay, taskbar thumbnail buttons,
//! the taskbar progress bar and tooltip, and (Windows) file associations. The
//! app talks to it only through the portable [`ShellIntegration`] trait, whose
//! signature names no backend or platform type; [`shell`] returns the right
//! implementation for the target and a [`NullShell`] when there is none.
//!
//! Keeping the seam here, and not in `app`, is what lets the app run on the
//! portable `xui_core` runtime on every target: `app` never sees `cfg(windows)`
//! for an OS service. The Windows implementation lives under `win` and is
//! compiled out entirely elsewhere, so this crate references no Windows-only
//! symbol on a non-Windows build.

mod shell;
mod window;

#[cfg(windows)]
mod win;

pub use shell::{NowPlaying, NullShell, ShellAction, ShellIntegration, ThumbButton, shell};
pub use window::NativeHandle;

#[cfg(windows)]
pub use win::{prepare, register_associations, unregister_associations};
