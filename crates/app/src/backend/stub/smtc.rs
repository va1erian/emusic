//! Non-Windows stub for [`crate::backend::smtc`] (#370).
//!
//! Windows System Media Transport Controls are a Windows integration; on every
//! other target these calls are no-ops so the rest of the app still compiles
//! against the same [`Smtc`] type.

use std::ffi::c_void;

use emusic_ui::library_api::LibraryDataSource;
use emusic_ui::player_api::PlayerApi;
use emusic_ui::state::AppState;

/// No-op stand-in for the Win32 SMTC integration.
pub struct Smtc;

impl Smtc {
    /// No-op: there is no OS media overlay outside Windows.
    pub fn new(_hwnd: Option<*mut c_void>) -> Self {
        Self
    }

    /// No-op.
    pub fn sync(
        &mut self,
        _player: &dyn PlayerApi,
        _library: &dyn LibraryDataSource,
        _state: &mut AppState,
    ) {
    }
}
