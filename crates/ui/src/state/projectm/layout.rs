//! Where the projectM visualization is shown (#300): docked in the
//! now-playing panel or in its own window, optionally fullscreen on a chosen
//! monitor, or hidden.

use serde::{Deserialize, Serialize};

use super::monitor::VizMonitor;

/// The non-fullscreen home of the visualization. Leaving fullscreen returns
/// here, so the frontend never has to remember a "previous" placement.
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
    Fullscreen,
}

/// Persisted visualization placement: shown or hidden, its dock, whether it
/// is fullscreen and on which monitor.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VizLayout {
    /// Whether the visualization is shown at all. Off by default: while
    /// hidden it does no work at all (#305).
    pub visible: bool,
    /// Where it lives when not fullscreen.
    pub dock: VizDock,
    /// Whether it covers a whole monitor.
    pub fullscreen: bool,
    /// The monitor fullscreen goes to, when one has been chosen. Resolved
    /// against the connected monitors with [`super::pick_monitor`].
    pub fullscreen_monitor: Option<VizMonitor>,
}

impl VizLayout {
    /// The surface to draw on, or `None` while hidden.
    pub fn surface(&self) -> Option<VizSurface> {
        if !self.visible {
            return None;
        }
        Some(match (self.fullscreen, self.dock) {
            (true, _) => VizSurface::Fullscreen,
            (false, VizDock::Panel) => VizSurface::Panel,
            (false, VizDock::Window) => VizSurface::Window,
        })
    }
}
