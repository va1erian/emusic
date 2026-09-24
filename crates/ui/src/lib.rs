#![forbid(unsafe_code)]

//! Toolkit-agnostic UI logic shared by emusic's frontends (#93, #97).
//!
//! Everything here is independent of any GUI toolkit: the [`PlayerApi`] and
//! [`LibraryDataSource`] traits frontends program against, the real and mock
//! [`backend`]s implementing them, the CLI, search, persisted-folder picking,
//! the frontend-agnostic [`waker`] and [`image_cache`] seams, the shared
//! [`state`], [`config`] and view-model pieces each frontend renders with its
//! own widgets, and the [`shell::Shell`] application controller that drives
//! them.
//!
//! The egui frontend (`crates/app`) depends on this crate and re-exports these
//! modules at their old paths, so UI code only changes `use` paths. The
//! egui-bound pieces (`backend::{smtc, thumbbar}`, theme/fonts) stay there.

pub mod auto_tag;
pub mod backend;
pub mod cli;
pub mod config;
pub mod folder_picker;
pub mod image_cache;
pub mod library_api;
pub mod mock;
pub mod panels;
pub mod player_api;
pub mod search;
pub mod shell;
pub mod state;
pub mod tag_editor;
pub mod views;
pub mod waker;
