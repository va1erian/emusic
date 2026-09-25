//! Windows shell integration for emusic: single instance detection with
//! named-pipe IPC ([`instance`]), per-user file association registration
//! ([`assoc`]), taskbar thumbnail-toolbar transport buttons ([`thumbbar`]) and
//! the DWM iconic taskbar thumbnail / progress bar ([`taskbar`]).
//!
//! This crate is library-only; wiring it into the `app` binary (argument
//! parsing, deciding when to register associations, etc.) is a separate
//! concern.

mod error;
mod sys;
mod taskbar_list;

pub mod assoc;
pub mod instance;
pub mod taskbar;
pub mod thumbbar;

pub use error::{Result, WinshellError};
pub use instance::{IpcMessage, Listener, SingleInstance};
pub use sys::{bring_to_front, is_remote_drive};
