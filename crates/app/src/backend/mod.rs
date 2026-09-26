//! OS integrations bound to this window's HWND (#320, #321, #322).
//!
//! The toolkit-agnostic backends (the real BASS/SQLite pair, the mock fakes,
//! the IPC core, the [`emusic_ui::waker`] seam) live in `emusic-ui`. Only the
//! pieces that bind to the window's HWND stay here: Windows System Media
//! Transport Controls ([`smtc`]), the taskbar thumbnail transport buttons
//! ([`thumbbar`]) and the DWM iconic taskbar preview / progress bar
//! ([`taskbar`]).
//!
//! All three are Windows shell integrations. On other targets a no-op stub with
//! the same API is compiled in instead ([`crate::backend`] stays cross-platform),
//! so the rest of the app does not have to know which OS it runs on.

#[cfg(windows)]
pub mod smtc;
#[cfg(windows)]
pub mod taskbar;
#[cfg(windows)]
pub mod thumbbar;

#[cfg(not(windows))]
#[path = "stub/smtc.rs"]
pub mod smtc;
#[cfg(not(windows))]
#[path = "stub/taskbar.rs"]
pub mod taskbar;
#[cfg(not(windows))]
#[path = "stub/thumbbar.rs"]
pub mod thumbbar;
