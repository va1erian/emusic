//! View models: toolkit-agnostic state and logic behind each view (#93).
//!
//! Each module holds plain data plus the rules that drive it (sorting,
//! selection, identity); rendering stays in the frontends. The egui
//! frontend re-exports these at its old `views::…` paths.

mod context;

pub mod album_grid;
pub mod artists;
pub mod column_browser;
pub mod folders;
pub mod genres;
pub mod history;
pub mod most_played;
pub mod music;
pub mod now_playing;
pub mod search_popup;
pub mod starred;
pub mod track_table;

pub use context::{Commands, Ctx};
