//! `emusic-client`: the cross-platform client for `emusic-server`.
//!
//! The crate is deliberately free of Win32, BASS, SID and SQLite so it builds
//! everywhere the desktop app will (Windows now, macOS later). It provides:
//!
//! - [`ServerEndpoint`] describing a server and deriving a stable id;
//! - [`CredentialStore`] holding the device keypair and access token;
//! - [`RemoteClient`] speaking the REST API (pair, refresh, sync, download);
//! - [`TrackCache`] materialising remote tracks into local files that the
//!   existing audio pipeline can open.
//!
//! All network calls are blocking (`ureq`), matching the app's thread-based
//! backends; callers run them on worker threads.

#![forbid(unsafe_code)]

pub mod auth;
pub mod cache;
pub mod client;
pub mod config;
pub mod credentials;
pub mod error;
pub mod types;
mod util;

pub use cache::TrackCache;
pub use client::RemoteClient;
pub use config::ServerEndpoint;
pub use credentials::{CredentialStore, Credentials};
pub use error::{ClientError, Result};
pub use types::{PairResponse, SyncDelta, TokenResponse, TrackView};
pub use util::unix_now;
