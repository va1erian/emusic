//! Choosing the fullscreen monitor (#300, #304): a saved [`VizMonitor`]
//! matched against the monitors connected now, with fallbacks.

use serde::{Deserialize, Serialize};

/// A monitor as persisted and as reported by the frontend. Matched by the
/// device name first (`\\.\DISPLAY2`) and then by the friendly name, since
/// device names can be reassigned when monitors are re-plugged.
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct VizMonitor {
    /// The OS device name, e.g. `\\.\DISPLAY2`.
    pub device: String,
    /// The human-readable name, e.g. `DELL U2720Q`.
    pub name: String,
}

/// Picks the fullscreen monitor among `connected` (in the frontend's order):
/// the saved one if still connected (by device name, then by friendly
/// name), else the monitor the main window is on, else the primary one.
/// Returns `None` only when `connected` is empty.
pub fn pick_monitor(
    saved: Option<&VizMonitor>,
    connected: &[VizMonitor],
    window_monitor: Option<usize>,
    primary: usize,
) -> Option<usize> {
    if connected.is_empty() {
        return None;
    }
    let saved_match = saved.and_then(|saved| {
        let by = |same: fn(&VizMonitor, &VizMonitor) -> bool| {
            connected.iter().position(|monitor| same(monitor, saved))
        };
        by(|a, b| !b.device.is_empty() && a.device == b.device)
            .or_else(|| by(|a, b| !b.name.is_empty() && a.name == b.name))
    });
    let in_range = |index: &usize| *index < connected.len();
    saved_match
        .or(window_monitor.filter(in_range))
        .or(Some(primary).filter(in_range))
        .or(Some(0))
}
