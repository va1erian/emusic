#![forbid(unsafe_code)]

//! emusic app shell: eframe/egui UI, mock data mode, and the traits
//! (`PlayerApi`, `LibraryDataSource`) that decouple the UI from real
//! backends. Split into a library so both the `emusic` binary and the
//! headless `emusic-shot` screenshot tool (behind the `shot` feature) can
//! share it.

pub mod app;
pub mod fonts;
pub mod library_api;
pub mod mock;
pub mod panels;
pub mod player_api;
pub mod state;
pub mod theme;
pub mod views;
