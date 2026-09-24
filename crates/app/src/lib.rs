#![forbid(unsafe_code)]

//! emusic app shell: eframe/egui UI on top of the toolkit-agnostic
//! `emusic-ui` crate (APIs, backends, mock data, view models). Split into a
//! library so both the `emusic` binary and the headless `emusic-shot`
//! screenshot tool (behind the `shot` feature) can share it.

pub mod app;
pub mod backend;
pub use emusic_ui::cli;
pub mod config;
pub mod fonts;
mod icons;
pub use emusic_ui::library_api;
pub use emusic_ui::mock;
pub mod panels;
pub use emusic_ui::player_api;
use emusic_ui::search;
pub mod settings;
pub mod shuffle;
pub mod state;
pub mod tag_editor;
pub mod theme;
pub mod views;
pub mod window_icon;
