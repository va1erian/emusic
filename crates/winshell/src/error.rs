//! Error types shared across the crate.

use std::io;

/// Errors returned by `winshell` operations.
#[derive(Debug, thiserror::Error)]
pub enum WinshellError {
    /// A Win32 API call, or the underlying named-pipe I/O, failed.
    #[error("Windows API error: {0}")]
    Io(#[from] io::Error),

    /// The IPC payload could not be (de)serialized.
    #[error("IPC message (de)serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A registry operation failed.
    #[error("registry error: {0}")]
    Registry(#[from] windows_result::Error),

    /// A named pipe connection attempt was refused / busy for too long.
    #[error("could not connect to the primary instance pipe: {0}")]
    PipeUnavailable(String),
}

/// Convenience alias for results returned by this crate.
pub type Result<T> = std::result::Result<T, WinshellError>;
