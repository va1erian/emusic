//! Safe, runtime-loaded wrapper over [libprojectM](https://github.com/projectM-visualizer/projectm)
//! 4.x, the open-source MilkDrop visualization engine (#295, #297).
//!
//! libprojectM is LGPL-2.1, so it is never linked at build time: the DLLs
//! are loaded at runtime with [`libloading`] from `$EMUSIC_PROJECTM_DIR` if
//! set, otherwise a `projectm\` folder next to the executable. The folder
//! holds `projectM-4.dll`, `projectM-4-playlist.dll` and, for the 4.1
//! releases, `glew32.dll`, which projectM resolves OpenGL through; this
//! crate initialises GLEW on the current context before creating an
//! instance. When the DLLs are absent [`ProjectM::load`] fails with
//! [`ProjectMError::DllNotFound`] and the frontend falls back to its
//! placeholder, so the workspace builds and tests without projectM.
//!
//! # Usage
//! Everything that touches OpenGL happens on the thread whose context is
//! current: create the [`Instance`] inside the frontend's GL paint, feed
//! it audio with [`Instance::add_pcm`], call [`Instance::render`] each
//! frame, and [`Instance::destroy`] it from a hook that runs with the same
//! context current. The instance remembers its context and refuses GL calls
//! ([`ProjectMError::WrongContext`]) when another one is current.
//!
//! Paths are passed to projectM as UTF-8. projectM opens files with the
//! narrow C runtime APIs, so non-ASCII paths only work when the host
//! process runs with the UTF-8 active code page (an app manifest setting).
//!
//! # Unsafe
//! All `unsafe` lives in the private `ffi` module, each block with a
//! `// SAFETY:` comment. Every other module starts with
//! `#![forbid(unsafe_code)]`.

pub mod error;
pub mod events;
mod ffi;
mod instance;
mod library;
pub mod params;

pub use error::ProjectMError;
pub use events::Event;
pub use ffi::loader::{CORE_DLL, GLEW_DLL, PLAYLIST_DLL, PROJECTM_DIR_ENV};
pub use instance::Instance;
pub use library::ProjectM;
pub use params::Parameters;

#[cfg(test)]
mod tests;
