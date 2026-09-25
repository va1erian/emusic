#![forbid(unsafe_code)]

//! A small CPU-side, audio-reactive stand-in for the projectM visualization.
//!
//! The real MilkDrop visuals come from libprojectM (#295), which renders
//! with OpenGL straight into the frontend's surface and so does not go
//! through this crate. [`PlaceholderEngine`] is what those surfaces draw
//! instead when projectM can't run (its libraries are missing, or no
//! OpenGL 3.3 context is available): a smooth plasma with no external
//! dependencies.
//!
//! [`MilkdropEngine`] is the trait a surface drives once per frame:
//! [`MilkdropEngine::feed_pcm`] with raw samples (the source the
//! oscilloscope reads via `PlayerApi::samples`), then
//! [`MilkdropEngine::tick`] to advance and read back a [`Frame`] to paint
//! with a plain 2D painter.

mod engine;
mod placeholder;

pub use engine::{Frame, MilkdropEngine};
pub use placeholder::PlaceholderEngine;
