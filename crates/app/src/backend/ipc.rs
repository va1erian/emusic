//! Single-instance detection and IPC wiring (#11), built on `winshell`.
//!
//! The frontend-agnostic core (`app_id`, [`IpcBridge`], [`resolve_paths`])
//! lives in `emusic-ui` and is re-exported here; the egui-bound repaint
//! callback that used to live alongside it is now the shared
//! [`Waker`](emusic_ui::waker::Waker) seam (#95), bound in `main.rs`.

pub use emusic_ui::backend::ipc::{IpcBridge, app_id, resolve_paths};
