//! Persistable window geometry (#214): the size, position and maximized
//! state the frontend should restore on the next launch.
//!
//! The app records the live window geometry here and applies the saved values
//! when it builds the window; the toolkit-agnostic shell only carries the value
//! through [`Config`](crate::config::Config).

use serde::{Deserialize, Serialize};

/// Last known window geometry. Every field is optional/`false` on a fresh
/// profile, so a first launch keeps the frontend's default window.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowGeometry {
    /// Inner size in logical points, if a normal (non-maximized) window has
    /// been measured.
    pub size: Option<[f32; 2]>,
    /// Outer top-left in screen coordinates, if known.
    pub position: Option<[f32; 2]>,
    /// Whether the window was maximized.
    pub maximized: bool,
}
