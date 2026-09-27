//! Win32 frontend's OS integrations (#320, #321, #322).
//!
//! The toolkit-agnostic backends (the real BASS/SQLite pair, the mock fakes,
//! the IPC core, the [`emusic_ui::waker`] seam) live in `emusic-ui`. Only the
//! pieces that bind to this window's HWND stay here: Windows System Media
//! Transport Controls ([`smtc`]), the taskbar thumbnail transport buttons
//! ([`thumbbar`]) and the DWM iconic taskbar preview / progress bar
//! ([`taskbar`]).

pub mod smtc;
pub mod taskbar;
pub mod thumbbar;
