//! Windows shell integration for emusic: single instance detection with
//! named-pipe IPC ([`instance`]), per-user file association registration
//! ([`assoc`]), and taskbar thumbnail-toolbar transport buttons ([`thumbbar`]).
//!
//! This crate is library-only; wiring it into the `app` binary (argument
//! parsing, deciding when to register associations, etc.) is a separate
//! concern.

mod error;
mod sys;

pub mod assoc;
pub mod instance;
pub mod thumbbar;

pub use error::{Result, WinshellError};
pub use instance::{IpcMessage, Listener, SingleInstance};
pub use sys::{bring_to_front, is_remote_drive};
