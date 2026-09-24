//! Persistent state for the right-hand now-playing panel (#97).
//!
//! The panel's artwork texture cache is egui-bound (it holds GPU handles) and
//! stays in the egui frontend; this is only the cross-frame state the shell
//! owns.

use crate::library_api::TrackInfo;

/// Persistent UI state for the now-playing panel.
#[derive(Default)]
pub struct PanelState {
    /// The track whose Properties dialog is open, if any. Rendered by the
    /// frontend so it works whichever view is active.
    pub properties: Option<TrackInfo>,
}
