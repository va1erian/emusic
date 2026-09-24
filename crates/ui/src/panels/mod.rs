//! Toolkit-agnostic persistent state for the fixed chrome panels (#97).
//!
//! The egui (and later Win32) frontends render the panels; only the state
//! that must survive across frames and be shared with config lives here.

pub mod now_playing;
pub mod visualizer;
