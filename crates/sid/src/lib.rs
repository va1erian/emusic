//! Safe Rust API over the vendored [cRSID](https://github.com/r-moeritz/crsid-by-hermit)
//! Commodore 64 SID engine.
//!
//! cRSID by Hermit (Mihály Horváth) is an integer-only, cycle-exact emulation
//! of the C64's 6502, CIA, VIC and SID chips, published under a permissive
//! WTFPL-style licence that requests attribution. It is vendored (unmodified)
//! under `vendor/crsid/` and compiled by `build.rs`; see
//! `vendor/crsid/README.md` for the upstream commit and licence text.
//!
//! This crate turns that C engine into a small safe API:
//! - [`SidHeader::parse`] reads the PSID/RSID header in plain Rust, so tune
//!   metadata and subtune counts need no engine at all.
//! - [`SidPlayer`] loads a tune from bytes, selects a subtune, applies
//!   chip-model/clock overrides and renders mono signed 16-bit PCM.
//!
//! # Example
//! ```no_run
//! # use emusic_sid::{SidConfig, SidPlayer};
//! # fn load() -> Vec<u8> { Vec::new() }
//! # fn main() -> Result<(), emusic_sid::SidError> {
//! let mut player = SidPlayer::from_bytes(load(), 44_100, SidConfig::default())?;
//! let mut pcm = vec![0i16; 1024];
//! player.render(&mut pcm);
//! # Ok(())
//! # }
//! ```
//!
//! # Unsafe
//! All `unsafe` in this crate lives in [`ffi`], which declares the C ABI and
//! wraps every call in a `// SAFETY:` comment. Every other module starts with
//! `#![forbid(unsafe_code)]`.

mod engine;
pub mod error;
mod ffi;
pub mod header;
pub mod hvsc;

pub use engine::{MAX_SAMPLE_RATE, MIN_SAMPLE_RATE, SidConfig, SidPlayer};
pub use error::SidError;
pub use header::{ChipModel, Clock, SidFormat, SidHeader};
pub use hvsc::{HvscError, HvscIndex};
