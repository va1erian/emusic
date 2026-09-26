//! Non-Windows stub for [`crate::backend::taskbar`] (#370).
//!
//! The DWM iconic taskbar preview and progress bar are a Windows shell
//! integration; on every other target these calls are no-ops so the rest of the
//! app still compiles against the same [`TaskbarPreview`] type.

use emusic_ui::player_api::PlayerApi;
use emusic_ui::views::now_playing::NowPlayingView;
use emusic_ui::waker::WakerHandle;

/// No-op stand-in for the Win32 taskbar preview integration.
pub struct TaskbarPreview;

impl TaskbarPreview {
    /// No-op: there is no DWM taskbar preview outside Windows.
    pub fn new(_hwnd: Option<isize>, _waker: WakerHandle) -> Self {
        Self
    }

    /// No-op.
    pub fn sync(&mut self, _player: &dyn PlayerApi, _model: &NowPlayingView) {}
}
