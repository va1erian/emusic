//! `emusic-mobile`: the Android-facing bindings over [`emusic_client`].
//!
//! The crate is a thin [uniffi] wrapper around the cross-platform client core,
//! so pairing, token refresh and sync cannot drift from the desktop. It owns no
//! Win32/BASS/SQLite code and builds on the host as well as for the Android
//! NDK, which keeps `cargo test` usable without a device.
//!
//! Kotlin owns the UI, the media session and storage policy; it constructs a
//! [`MobileCore`] with the app's private data directory and calls its exported
//! methods. Media playback itself is handled by Media3, which only needs the
//! URLs (and the bearer token) this crate produces.

#![forbid(unsafe_code)]

uniffi::setup_scaffolding!();

mod core;
mod error;
mod library_store;
mod registry;
mod types;
mod util;

pub use core::{MobileCore, list_servers};
pub use error::MobileError;
pub use types::{AuthState, Health, ServerEntry, SyncResult, Track};
