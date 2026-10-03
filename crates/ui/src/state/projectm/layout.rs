//! Where the projectM visualization is shown (#300, #515): docked in the
//! now-playing panel or in its own window, or hidden. There is no fullscreen
//! mode; the user maximizes the window instead.

use serde::{Deserialize, Serialize};

/// Where the visualization lives while shown. Double-clicking the surface
/// moves it to the other one (#515).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VizDock {
    /// A section under the now-playing summary in the right panel.
    #[default]
    Panel,
    /// An independent, resizable window.
    Window,
}

/// The surface the visualization is currently drawn on, derived from a
/// [`VizLayout`]. There is exactly one at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VizSurface {
    Panel,
    Window,
}

/// Persisted visualization placement: shown or hidden, and its dock.
///
/// Deserialized through [`StoredVizLayout`], so a config written before
/// fullscreen was removed (#515) still loads: `fullscreen = true` maps to the
/// window dock and the old monitor choice is dropped.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "StoredVizLayout")]
pub struct VizLayout {
    /// Whether the visualization is shown at all. Off by default: while
    /// hidden it does no work at all (#305).
    pub visible: bool,
    /// Where it is shown.
    pub dock: VizDock,
}

impl VizLayout {
    /// The surface to draw on, or `None` while hidden.
    pub fn surface(&self) -> Option<VizSurface> {
        if !self.visible {
            return None;
        }
        Some(match self.dock {
            VizDock::Panel => VizSurface::Panel,
            VizDock::Window => VizSurface::Window,
        })
    }
}

/// The on-disk shape of [`VizLayout`], including the retired `fullscreen`
/// flag. Unknown keys (such as the old `fullscreen_monitor`) are ignored.
#[derive(Deserialize, Default)]
#[serde(default)]
struct StoredVizLayout {
    visible: bool,
    dock: VizDock,
    fullscreen: bool,
}

impl From<StoredVizLayout> for VizLayout {
    fn from(stored: StoredVizLayout) -> Self {
        Self {
            visible: stored.visible,
            dock: if stored.fullscreen {
                VizDock::Window
            } else {
                stored.dock
            },
        }
    }
}
