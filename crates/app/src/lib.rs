#![forbid(unsafe_code)]

//! emusic app shell: eframe/egui UI, mock data mode, and the traits
//! (`PlayerApi`, `LibraryDataSource`) that decouple the UI from real
//! backends. Split into a library so both the `emusic` binary and the
//! headless `emusic-shot` screenshot tool (behind the `shot` feature) can
//! share it.

pub mod app;
pub mod backend;
pub mod cli;
pub mod config;
pub mod fonts;
mod icons;
pub mod library_api;
pub mod mock;
pub mod panels;
pub mod player_api;
mod search;
pub mod settings;
pub mod shuffle;
pub mod state;
pub mod theme;
pub mod views;
