//! Egui frontend's backend wiring (#11, #54).
//!
//! The toolkit-agnostic backends (real BASS/SQLite pair, mock fakes, IPC
//! core) live in `emusic-ui` and are re-exported here; only the egui-bound
//! pieces stay: [`ipc::RepaintHandle`], [`smtc`] (system media transport) and
//! [`thumbbar`] (taskbar buttons).

pub mod ipc;
pub mod smtc;
pub mod thumbbar;

pub use emusic_ui::backend::{Backends, build, library, player_adapter};
