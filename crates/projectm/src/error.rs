#![forbid(unsafe_code)]

//! [`ProjectMError`]: everything loading or driving projectM can fail with.

use thiserror::Error;

/// Errors from loading the projectM libraries or driving an instance.
#[derive(Debug, Error)]
pub enum ProjectMError {
    /// A required DLL is missing or the OS loader rejected it.
    #[error("projectM library not found: {0}")]
    DllNotFound(String),
    /// A DLL loaded but lacks an entry point this crate needs (wrong
    /// version).
    #[error("projectM library is missing the `{0}` entry point")]
    MissingSymbol(String),
    /// No OpenGL context is current on this thread.
    #[error("no OpenGL context is current on this thread")]
    NoCurrentContext,
    /// The current OpenGL context is not the one the instance was created
    /// with.
    #[error("a different OpenGL context is current than the instance was created with")]
    WrongContext,
    /// GLEW could not initialise against the current context.
    #[error("GLEW failed to initialise (code {0})")]
    GlewInit(u32),
    /// projectM refused to create an instance or a playlist.
    #[error("projectM failed to create {0}")]
    CreateFailed(&'static str),
    /// A path or string can't be passed to projectM (not UTF-8, or contains
    /// a NUL byte).
    #[error("cannot pass {0:?} to projectM")]
    InvalidString(String),
}
