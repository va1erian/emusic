#![forbid(unsafe_code)]

//! Toolkit-agnostic UI logic shared by emusic's frontends (#93).
//!
//! Everything here is independent of any GUI toolkit: the [`PlayerApi`] and
//! [`LibraryDataSource`] traits frontends program against, the real and mock
//! [`backend`]s implementing them, the CLI, search, persisted-folder picking,
//! and the view-model pieces (sorting, selection, album identity) that each
//! frontend renders with its own widgets.
//!
//! The egui frontend (`crates/app`) depends on this crate and re-exports these
//! modules at their old paths, so UI code only changes `use` paths. Still
//! frontend-side: `config` (needs the `AppState` aggregate), `shuffle`,
//! `app::commands` and the `AppState` shell state itself (until the view
//! states move over in #98–#104), plus the egui-bound
//! `backend::{smtc,thumbbar}`.

pub mod backend;
pub mod cli;
pub mod folder_picker;
pub mod library_api;
pub mod mock;
pub mod player_api;
pub mod search;
pub mod state;
pub mod views;
