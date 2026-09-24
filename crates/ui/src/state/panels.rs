//! Visibility of the shell's optional panels and the [`PanelKind`] used to
//! toggle them.

use serde::{Deserialize, Serialize};

/// Which optional panels are visible (toggled from the View menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelVisibility {
    pub navigator: bool,
    pub right_panel: bool,
    pub status_bar: bool,
}

impl Default for PanelVisibility {
    fn default() -> Self {
        Self {
            navigator: true,
            right_panel: true,
            status_bar: true,
        }
    }
}

/// Identifies one optional panel for [`Command::TogglePanel`].
///
/// [`Command::TogglePanel`]: crate::state::Command::TogglePanel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    Navigator,
    RightPanel,
    StatusBar,
}
