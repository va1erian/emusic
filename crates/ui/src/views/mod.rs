//! View models: toolkit-agnostic state and logic behind each view (#93).
//!
//! Each module holds plain data plus the rules that drive it (sorting,
//! selection, identity); rendering stays in the frontends. The egui
//! frontend re-exports these at its old `views::…` paths.

pub mod album_grid;
pub mod column_browser;
pub mod folder_tree;
pub mod history;
pub mod most_played;
pub mod track_table;
