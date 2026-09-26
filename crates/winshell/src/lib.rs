//! Windows shell integration for emusic: single instance detection with
//! named-pipe IPC ([`instance`]), per-user file association registration
//! ([`assoc`]), taskbar thumbnail-toolbar transport buttons ([`thumbbar`]),
//! the DWM iconic taskbar thumbnail / progress bar ([`taskbar`]) and the
//! focused-control query backing bare-key shortcuts ([`input`]).
//!
//! This crate is library-only; wiring it into the `app` binary (argument
//! parsing, deciding when to register associations, etc.) is a separate
//! concern.

mod error;
mod sys;

#[cfg(windows)]
mod taskbar_list;

#[cfg(windows)]
pub mod assoc;
#[cfg(windows)]
pub mod input;
#[cfg(windows)]
pub mod taskbar;
#[cfg(windows)]
pub mod thumbbar;

pub use error::{Result, WinshellError};
pub use sys::{bring_to_front, is_remote_drive};

#[cfg(windows)]
pub mod instance;

#[cfg(not(windows))]
pub mod instance {
    use super::*;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct IpcMessage {
        pub enqueue: bool,
        pub files: Vec<std::path::PathBuf>,
        pub cwd: std::path::PathBuf,
    }

    #[derive(Debug)]
    pub struct Listener;

    pub enum SingleInstance {
        Primary(Listener),
        Secondary,
    }

    impl SingleInstance {
        pub fn acquire(
            _app_id: &str,
            _waker: impl Fn() + Send + Sync + 'static,
        ) -> std::io::Result<Self> {
            Ok(Self::Primary(Listener))
        }
    }

    pub fn send_to_primary(_app_id: &str, _message: &IpcMessage) -> Result<()> {
        Ok(())
    }
}

pub use instance::{IpcMessage, Listener, SingleInstance};
