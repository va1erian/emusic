#![forbid(unsafe_code)]

//! Toolkit-agnostic UI logic behind emusic (#93, #97).
//!
//! Everything here is independent of any GUI toolkit: the [`PlayerApi`] and
//! [`LibraryDataSource`] traits the app programs against, the real and mock
//! [`backend`]s implementing them, the CLI, search, persisted-folder picking,
//! the toolkit-agnostic [`waker`] and [`image_cache`] seams, the shared
//! [`state`], [`config`] and view-model pieces the app renders with its own
//! widgets, and the [`shell::Shell`] application controller that drives them.
//!
//! The application (`crates/app`) depends on this crate: it renders the shared
//! state and view models with native Win32 controls. The toolkit-bound pieces
//! (`backend::{smtc, thumbbar}`, theme/fonts) live there.

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
pub mod remote;
pub mod search;
pub mod shell;
pub mod state;
pub mod tag_editor;
pub mod views;
pub mod waker;
