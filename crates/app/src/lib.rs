#![forbid(unsafe_code)]

//! emusic's egui frontend: eframe/egui rendering on top of the
//! toolkit-agnostic `emusic-ui` crate (APIs, backends, state, shell, view
//! models). Split into a library so both the `emusic` binary and the headless
//! `emusic-shot` screenshot tool (behind the `shot` feature) can share it.

pub mod app;
pub mod appearance;
pub mod backend;
pub use emusic_ui::cli;
pub use emusic_ui::config;
pub mod fonts;
mod icons;
pub mod image_sink;
pub use emusic_ui::library_api;
pub use emusic_ui::mock;
pub mod panels;
pub use emusic_ui::player_api;
use emusic_ui::search;
pub mod settings;
pub mod shuffle;
pub use emusic_ui::state;
pub mod tag_editor;
pub mod theme;
pub mod views;
pub use emusic_ui::{shell, waker};
pub mod window_icon;
