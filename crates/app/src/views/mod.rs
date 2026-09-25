//! The app's views (#106).
//!
//! Each region of the main window is a view: a struct owning the controls it
//! draws into, a `sync` that updates them from the shell's state, and — for
//! the central area — a message type mapped from control notifications. The
//! first slice puts placeholders where the real views will go.

pub mod album_grid;
pub mod artists;
pub mod column_browser;
pub mod folders;
pub mod genres;
pub mod history;
pub mod most_played;
pub mod music;
pub mod name_counts;
pub mod navigator;
pub mod now_playing;
pub mod placeholder;
pub mod projectm;
pub mod settings;
pub mod starred;
pub mod status_bar;
pub mod top_bar;
pub mod track_table;
pub mod visualizer;
