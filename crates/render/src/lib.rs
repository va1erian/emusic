#![forbid(unsafe_code)]

//! Server-side rendering of specialized audio to PCM, encoded for Android.
//!
//! The `emusic` desktop client plays SID, tracker modules and MIDI with its
//! native engines. Android has no such engines (and cRSID will not build for
//! the NDK), so `emusic-server` renders those formats to a codec Android can
//! decode — currently FLAC — and serves the result.
//!
//! This crate is deliberately transport-agnostic: it turns a specialized file
//! into PCM and encodes it, and offers a rendition cache. Wiring it into the
//! `/render` HTTP endpoint is a separate increment.
//!
//! # Layers
//! - [`Renderer`] is the abstraction over a format family. [`SidRenderer`] is
//!   the first implementation; tracker-module and MIDI renderers can be added
//!   later without changing this API.
//! - [`PcmBuffer`] is the renderers' common output.
//! - [`flac::encode`] turns PCM into FLAC bytes with the pure-Rust `flacenc`.
//! - [`RenditionCache`] stores encoded renditions under a caller-provided root,
//!   keyed deterministically and capped by an LRU size budget.

mod cache;
mod codec;
pub mod error;
pub mod flac;
#[cfg(feature = "modules")]
mod modules;
mod pcm;
mod renderer;
#[cfg(feature = "sid")]
mod sid;

pub use cache::{RenditionCache, RenditionKey};
pub use codec::Codec;
pub use error::RenderError;
#[cfg(feature = "modules")]
pub use modules::{MODULE_EXTENSIONS, ModuleRenderer};
pub use pcm::PcmBuffer;
pub use renderer::{RenderOptions, Renderer};
#[cfg(feature = "sid")]
pub use sid::SidRenderer;
