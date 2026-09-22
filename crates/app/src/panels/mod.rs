//! Fixed chrome around the central view: top transport bar, left navigator,
//! right now-playing panel, bottom status bar. Each panel is a thin
//! `show(ui, state, ...)` function that only pushes [`crate::state::Command`]s
//! rather than mutating state directly, so panels never need to know about
//! each other.

pub mod navigator;
pub mod now_playing;
pub mod right_panel;
pub mod search_popup;
pub mod status_bar;
pub mod top_bar;
