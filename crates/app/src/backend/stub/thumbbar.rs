//! Non-Windows stub for [`crate::backend::thumbbar`] (#370).
//!
//! Taskbar thumbnail-toolbar buttons are a Windows shell integration; on every
//! other target these calls are no-ops so the rest of the app still compiles
//! against the same [`ThumbBar`] type.

use emusic_ui::player_api::PlayerApi;
use emusic_ui::state::AppState;

/// No-op stand-in for the Win32 taskbar thumbnail-toolbar integration.
pub struct ThumbBar;

impl ThumbBar {
    /// No-op: there is no taskbar thumbnail toolbar outside Windows.
    pub fn new(_hwnd: Option<isize>) -> Self {
        Self
    }

    /// No-op.
    pub fn sync(&mut self, _player: &dyn PlayerApi, _state: &mut AppState) {}
}
