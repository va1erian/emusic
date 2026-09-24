//! Egui frontend's backend wiring (#11, #54).
//!
//! The toolkit-agnostic backends (real BASS/SQLite pair, mock fakes, IPC
//! core, the [`emusic_ui::waker`] seam) live in `emusic-ui` and are
//! re-exported here; only the egui-bound pieces stay: [`waker::EguiWaker`],
//! [`smtc`] (system media transport) and [`thumbbar`] (taskbar buttons).

pub mod ipc;
pub mod smtc;
pub mod thumbbar;
pub mod waker;

pub use emusic_ui::backend::{Backends, build, library, player_adapter};
