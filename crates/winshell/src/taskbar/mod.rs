//! DWM iconic taskbar thumbnail, progress bar and thumbnail tooltip (#322).
//!
//! Windows can replace the taskbar's default screenshot preview with an
//! app-drawn "iconic" card. This module mirrors the transport state onto it:
//!
//! * [`TaskBar::new`] turns on `DWMWA_FORCE_ICONIC_REPRESENTATION` +
//!   `DWMWA_HAS_ICONIC_BITMAP`, which makes DWM send
//!   `WM_DWMSENDICONICTHUMBNAIL` when it needs the card.
//! * [`msg_hook`] records the requested maximum size; the app later renders a
//!   [`Panel`] with [`TaskBar::set_iconic_thumbnail`], which GDI-draws it into
//!   a 32bpp DIB (cover, title, artist, album, `elapsed / total`) and hands it
//!   to `DwmSetIconicThumbnail`.
//! * [`TaskBar::invalidate_iconic`] tells DWM the card is stale (a new track,
//!   a finished cover decode, play/pause, or the next whole second), so it
//!   re-requests one.
//! * [`TaskBar::set_progress`] shows the playback progress on the taskbar
//!   button, and [`TaskBar::set_tooltip`] the hover tooltip.
//!
//! `WM_DWMSENDICONICLIVEPREVIEWBITMAP` (Aero Peek) is deliberately *not*
//! claimed: answering it would need a second, larger render on every peek, and
//! leaving it to `DefWindowProc` keeps DWM's default preview. The iconic card
//! covers the hover case this issue is about.
//!
//! The progress bar could borrow the toolbar's `ITaskbarList3` (see
//! [`crate::thumbbar`]) instead of creating a second one. It does not: the two
//! features attach and can fail independently, the coclass is cheap to
//! instantiate, and sharing would couple the modules through a COM handle.
//!
//! This is one of the crate's raw Win32/COM surfaces: every `unsafe` block
//! carries a `// SAFETY:` comment and the public API is safe. Creation
//! degrades to a [`Result::Err`] (and the app to a logged no-op) when COM, the
//! shell object or DWM is unavailable.

mod layout;
mod paint;

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::Mutex;

#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, HWND};
#[cfg(windows)]
use windows::Win32::Graphics::Dwm::{
    DWMWA_FORCE_ICONIC_REPRESENTATION, DWMWA_HAS_ICONIC_BITMAP, DwmInvalidateIconicBitmaps,
    DwmSetIconicThumbnail, DwmSetWindowAttribute,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    ITaskbarList3, TBPF_NOPROGRESS, TBPF_NORMAL, TBPF_PAUSED, TBPFLAG,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{MSG, WM_DWMSENDICONICTHUMBNAIL};

use crate::Result;
#[cfg(windows)]
use crate::taskbar_list::{self, to_io};

pub use layout::{Cover, Panel, ThumbnailSize};

static THUMBNAIL_REQUEST: Mutex<Option<ThumbnailSize>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    None,
    Normal,
    Paused,
}

#[cfg(windows)]
pub fn msg_hook(msg: *const c_void) -> bool {
    let msg = unsafe { &*msg.cast::<MSG>() };
    if msg.message != WM_DWMSENDICONICTHUMBNAIL {
        return false;
    }
    let packed = msg.lParam.0 as usize;
    let size = ThumbnailSize {
        width: ((packed >> 16) & 0xffff) as u32,
        height: (packed & 0xffff) as u32,
    };
    *THUMBNAIL_REQUEST
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(size);
    true
}

#[cfg(not(windows))]
pub fn msg_hook(_msg: *const c_void) -> bool {
    false
}

pub fn take_thumbnail_request() -> Option<ThumbnailSize> {
    THUMBNAIL_REQUEST
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

#[cfg(windows)]
pub struct TaskBar {
    taskbar: ITaskbarList3,
    hwnd: HWND,
    iconic: bool,
    last_progress: Option<(ProgressState, u64, u64)>,
    last_tooltip: Option<String>,
}

#[cfg(not(windows))]
pub struct TaskBar;

#[cfg(windows)]
impl TaskBar {
    pub fn new(hwnd: isize) -> Result<Self> {
        let (taskbar, hwnd) = taskbar_list::create(hwnd)?;
        let iconic = enable_iconic(hwnd);
        Ok(Self {
            taskbar,
            hwnd,
            iconic,
            last_progress: None,
            last_tooltip: None,
        })
    }

    #[must_use]
    pub fn iconic_enabled(&self) -> bool {
        self.iconic
    }

    pub fn set_iconic_thumbnail(&self, size: ThumbnailSize, panel: &Panel<'_>) -> Result<()> {
        if !self.iconic {
            return Ok(());
        }
        let bitmap = paint::render(size, panel)?;
        let result = unsafe { DwmSetIconicThumbnail(self.hwnd, bitmap, 0) };
        unsafe {
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(bitmap);
        }
        result.map_err(to_io)
    }

    pub fn invalidate_iconic(&self) -> Result<()> {
        if !self.iconic {
            return Ok(());
        }
        unsafe { DwmInvalidateIconicBitmaps(self.hwnd) }.map_err(to_io)
    }

    pub fn set_tooltip(&mut self, tooltip: &str) -> Result<()> {
        if self.last_tooltip.as_deref() == Some(tooltip) {
            return Ok(());
        }
        let wide = windows::core::HSTRING::from(tooltip);
        unsafe { self.taskbar.SetThumbnailTooltip(self.hwnd, &wide) }.map_err(to_io)?;
        self.last_tooltip = Some(tooltip.to_string());
        Ok(())
    }

    pub fn set_progress(
        &mut self,
        state: ProgressState,
        completed_secs: u64,
        total_secs: u64,
    ) -> Result<()> {
        if self.last_progress == Some((state, completed_secs, total_secs)) {
            return Ok(());
        }
        unsafe {
            self.taskbar
                .SetProgressState(self.hwnd, progress_flags(state))
        }
        .map_err(to_io)?;
        if state != ProgressState::None {
            unsafe {
                self.taskbar
                    .SetProgressValue(self.hwnd, completed_secs, total_secs.max(1))
            }
            .map_err(to_io)?;
        }
        self.last_progress = Some((state, completed_secs, total_secs));
        Ok(())
    }
}

#[cfg(not(windows))]
impl TaskBar {
    pub fn new(_hwnd: isize) -> Result<Self> {
        Err(crate::WinshellError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Not supported on non-windows",
        )))
    }

    #[must_use]
    pub fn iconic_enabled(&self) -> bool {
        false
    }

    pub fn set_iconic_thumbnail(&self, _size: ThumbnailSize, _panel: &Panel<'_>) -> Result<()> {
        Ok(())
    }

    pub fn invalidate_iconic(&self) -> Result<()> {
        Ok(())
    }

    pub fn set_tooltip(&mut self, _tooltip: &str) -> Result<()> {
        Ok(())
    }

    pub fn set_progress(
        &mut self,
        _state: ProgressState,
        _completed_secs: u64,
        _total_secs: u64,
    ) -> Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
fn progress_flags(state: ProgressState) -> TBPFLAG {
    match state {
        ProgressState::None => TBPF_NOPROGRESS,
        ProgressState::Normal => TBPF_NORMAL,
        ProgressState::Paused => TBPF_PAUSED,
    }
}

#[cfg(windows)]
fn enable_iconic(hwnd: HWND) -> bool {
    let enabled = BOOL(1);
    let value = std::ptr::from_ref(&enabled).cast::<c_void>();
    let size = size_of::<BOOL>() as u32;
    let forced =
        unsafe { DwmSetWindowAttribute(hwnd, DWMWA_FORCE_ICONIC_REPRESENTATION, value, size) };
    let has_bitmap = unsafe { DwmSetWindowAttribute(hwnd, DWMWA_HAS_ICONIC_BITMAP, value, size) };
    forced.is_ok() && has_bitmap.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thumbnail_message(width: u32, height: u32) -> MSG {
        MSG {
            message: WM_DWMSENDICONICTHUMBNAIL,
            // HIWORD carries the width, LOWORD the height.
            lParam: windows::Win32::Foundation::LPARAM(
                (((width as usize) << 16) | (height as usize & 0xffff)) as isize,
            ),
            ..MSG::default()
        }
    }

    fn dispatch(msg: &MSG) -> bool {
        msg_hook((msg as *const MSG).cast::<c_void>())
    }

    #[test]
    fn thumbnail_request_is_claimed_and_consumed_once() {
        let _ = take_thumbnail_request();
        assert!(
            take_thumbnail_request().is_none(),
            "nothing pending to start"
        );

        assert!(dispatch(&thumbnail_message(320, 180)));
        assert_eq!(
            take_thumbnail_request(),
            Some(ThumbnailSize {
                width: 320,
                height: 180
            })
        );
        assert!(take_thumbnail_request().is_none(), "consuming clears it");
    }

    #[test]
    fn unrelated_and_live_preview_messages_are_not_claimed() {
        assert!(!dispatch(&MSG::default()), "a non-DWM message is ignored");
        let live_preview = MSG {
            message: windows::Win32::UI::WindowsAndMessaging::WM_DWMSENDICONICLIVEPREVIEWBITMAP,
            ..MSG::default()
        };
        assert!(
            !dispatch(&live_preview),
            "live preview stays with DWM's default handling"
        );
        assert!(take_thumbnail_request().is_none());
    }

    #[test]
    fn progress_states_map_to_the_shell_flags() {
        assert_eq!(progress_flags(ProgressState::None), TBPF_NOPROGRESS);
        assert_eq!(progress_flags(ProgressState::Normal), TBPF_NORMAL);
        assert_eq!(progress_flags(ProgressState::Paused), TBPF_PAUSED);
    }
}
