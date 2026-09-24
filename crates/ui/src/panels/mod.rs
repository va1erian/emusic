//! Toolkit-agnostic persistent state for the fixed chrome panels (#97).
//!
//! The egui (and later Win32) frontends render the panels; only the state
//! that must survive across frames and be shared with config lives here.

pub mod navigator;
pub mod status_bar;
pub mod top_bar;
pub mod visualizer;
