#![forbid(unsafe_code)]

//! Toolkit-agnostic visualization-engine seam for a future MilkDrop/projectM
//! integration.
//!
//! [`MilkdropEngine`] is the trait a frontend panel drives once per frame:
//! [`MilkdropEngine::feed_pcm`] with raw samples (the same source the
//! existing oscilloscope reads via `PlayerApi::samples`), then
//! [`MilkdropEngine::tick`] to advance and read back a [`Frame`] to paint.
//! [`PlaceholderEngine`] is the only implementation shipped here: a small
//! audio-reactive plasma with no external dependencies, standing in for a
//! real engine.
//!
//! A real binding (e.g. `projectm-rs`/`projectm-sys` against libprojectM)
//! would live in its own crate implementing this same trait and feeding a
//! GPU-texture-backed `Frame` instead of the placeholder's plain numbers —
//! only this crate and the panel that paints [`Frame`] would need to change
//! to offer it as an alternative to [`PlaceholderEngine`]; the audio path
//! and the frontend/backend split stay as they are. It isn't included here:
//! it needs real `unsafe` FFI against a vendored or runtime-loaded native
//! library (mirroring how the `bass` crate isolates its FFI), which is out
//! of scope for this prototype.

mod engine;
mod placeholder;

pub use engine::{Frame, MilkdropEngine};
pub use placeholder::PlaceholderEngine;
