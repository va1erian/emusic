//! Win32 frontend's OS integrations (#320, #321).
//!
//! The toolkit-agnostic backends (the real BASS/SQLite pair, the mock fakes,
//! the IPC core, the [`emusic_ui::waker`] seam) live in `emusic-ui`. Only the
//! pieces that bind to this window's HWND stay here: Windows System Media
//! Transport Controls ([`smtc`]) and the taskbar thumbnail transport buttons
//! ([`thumbbar`]).

pub mod smtc;
pub mod thumbbar;
