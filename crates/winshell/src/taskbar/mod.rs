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

use windows::Win32::Foundation::{BOOL, HWND};
use windows::Win32::Graphics::Dwm::{
    DWMWA_FORCE_ICONIC_REPRESENTATION, DWMWA_HAS_ICONIC_BITMAP, DwmInvalidateIconicBitmaps,
    DwmSetIconicThumbnail, DwmSetWindowAttribute,
};
use windows::Win32::UI::Shell::{
    ITaskbarList3, TBPF_NOPROGRESS, TBPF_NORMAL, TBPF_PAUSED, TBPFLAG,
};
use windows::Win32::UI::WindowsAndMessaging::{MSG, WM_DWMSENDICONICTHUMBNAIL};

use crate::Result;
use crate::taskbar_list::{self, to_io};

pub use layout::{Cover, Panel, ThumbnailSize};

/// The size DWM last asked for, filled by [`msg_hook`] and consumed by
/// [`take_thumbnail_request`].
static THUMBNAIL_REQUEST: Mutex<Option<ThumbnailSize>> = Mutex::new(None);

/// The progress a [`TaskBar`] shows for the current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    /// No progress bar (`TBPF_NOPROGRESS`): stopped, or nothing loaded.
    None,
    /// A determinate bar (`TBPF_NORMAL`): playing.
    Normal,
    /// A yellow bar (`TBPF_PAUSED`): paused.
    Paused,
}

/// winit-style message hook for the iconic-thumbnail messages.
///
/// Intended as the callback for `Ui::on_raw_message` (or winit's equivalent),
/// which guarantees `msg` points to a live `MSG` for the duration of the call;
/// that contract is why this can stay safe despite taking a raw pointer.
/// Returns `true` for the messages it claims — the thumbnail request, recorded
/// for [`take_thumbnail_request`] and the app woken to answer it — and `false`
/// otherwise, so a live-preview request still reaches `DefWindowProc`.
pub fn msg_hook(msg: *const c_void) -> bool {
    // SAFETY: `msg` is the caller's pointer to the `MSG` it is about to
    // dispatch (and so is aligned and valid for this call). Our `MSG` and
    // win32ui's are both `#[repr(C)]` with identical fields, so the
    // reinterpretation is layout-compatible. We only read it.
    let msg = unsafe { &*msg.cast::<MSG>() };
    if msg.message != WM_DWMSENDICONICTHUMBNAIL {
        return false;
    }
    // Per the message contract the HIWORD is the maximum width (x) and the
    // LOWORD the maximum height (y); DWM rejects a thumbnail larger than
    // either, so getting them the wrong way round fails every request.
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

/// Consumes the size DWM last asked to render, if any.
///
/// Whenever this returns `Some`, the caller should render a [`Panel`] at that
/// size and call [`TaskBar::set_iconic_thumbnail`].
pub fn take_thumbnail_request() -> Option<ThumbnailSize> {
    THUMBNAIL_REQUEST
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

/// The taskbar integration: the shell object, the DWM iconic-bitmap state and
/// the last values pushed, so redundant shell/DWM calls are skipped.
///
/// Lives on the UI thread for as long as the app: the COM interface is not
/// `Send`.
pub struct TaskBar {
    taskbar: ITaskbarList3,
    hwnd: HWND,
    /// Whether DWM accepted the iconic-bitmap attributes. `false` (e.g. DWM
    /// composition off) turns the thumbnail methods into no-ops; DWM then never
    /// requests one.
    iconic: bool,
    /// Last `(state, completed, total)`, so the ~1 Hz progress updates only
    /// touch the shell when they actually change.
    last_progress: Option<(ProgressState, u64, u64)>,
    /// Last tooltip text, to skip redundant `SetThumbnailTooltip` calls.
    last_tooltip: Option<String>,
}

impl TaskBar {
    /// Creates the shell object for `hwnd` and enables DWM's iconic
    /// representation. The card itself is rendered when DWM requests it (see
    /// [`msg_hook`] / [`take_thumbnail_request`]).
    ///
    /// `hwnd` is the raw window handle. Returns an error when COM or the shell
    /// object cannot be created (e.g. Explorer is not running); a DWM refusal
    /// only disables the card, leaving the progress bar working.
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

    /// Whether DWM accepted the iconic-bitmap attributes (the custom card is
    /// active). `false` means DWM keeps its default preview.
    #[must_use]
    pub fn iconic_enabled(&self) -> bool {
        self.iconic
    }

    /// Renders `panel` at `size` and publishes it as the taskbar preview.
    ///
    /// A no-op when the iconic card could not be enabled. The bitmap is
    /// destroyed after DWM has copied it.
    pub fn set_iconic_thumbnail(&self, size: ThumbnailSize, panel: &Panel<'_>) -> Result<()> {
        if !self.iconic {
            return Ok(());
        }
        let bitmap = paint::render(size, panel)?;
        // SAFETY: `hwnd` is the live window; `bitmap` is a 32bpp DIB section
        // created by `paint::render` and owned by us.
        let result = unsafe { DwmSetIconicThumbnail(self.hwnd, bitmap, 0) };
        // SAFETY: DWM copies the pixels and does not take ownership, so the
        // bitmap is destroyed here exactly once, whichever way the call went.
        unsafe {
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(bitmap);
        }
        result.map_err(to_io)
    }

    /// Tells DWM the published card is stale, so it re-requests one.
    ///
    /// A no-op when the iconic card could not be enabled.
    pub fn invalidate_iconic(&self) -> Result<()> {
        if !self.iconic {
            return Ok(());
        }
        // SAFETY: `hwnd` is the live window; the call takes no pointers.
        unsafe { DwmInvalidateIconicBitmaps(self.hwnd) }.map_err(to_io)
    }

    /// Sets the hover tooltip on the taskbar button; redundant text is
    /// skipped.
    pub fn set_tooltip(&mut self, tooltip: &str) -> Result<()> {
        if self.last_tooltip.as_deref() == Some(tooltip) {
            return Ok(());
        }
        let wide = windows::core::HSTRING::from(tooltip);
        // SAFETY: `hwnd` is the live window; `wide` is a valid string for the
        // duration of the call.
        unsafe { self.taskbar.SetThumbnailTooltip(self.hwnd, &wide) }.map_err(to_io)?;
        self.last_tooltip = Some(tooltip.to_string());
        Ok(())
    }

    /// Shows `state`'s progress with `completed_secs` / `total_secs`.
    ///
    /// Redundant `(state, completed, total)` updates are skipped, which is
    /// what keeps this at the ~1 Hz the caller drives it at. `total_secs` is
    /// floored at 1 so a zero-length track cannot divide by zero in the shell.
    pub fn set_progress(
        &mut self,
        state: ProgressState,
        completed_secs: u64,
        total_secs: u64,
    ) -> Result<()> {
        if self.last_progress == Some((state, completed_secs, total_secs)) {
            return Ok(());
        }
        // SAFETY: `hwnd` is the live window; the flags are plain values.
        unsafe {
            self.taskbar
                .SetProgressState(self.hwnd, progress_flags(state))
        }
        .map_err(to_io)?;
        if state != ProgressState::None {
            // SAFETY: `hwnd` is the live window; both values are plain.
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

/// Maps a [`ProgressState`] to the shell's `TBPFLAG`.
fn progress_flags(state: ProgressState) -> TBPFLAG {
    match state {
        ProgressState::None => TBPF_NOPROGRESS,
        ProgressState::Normal => TBPF_NORMAL,
        ProgressState::Paused => TBPF_PAUSED,
    }
}

/// Enables DWM's iconic representation, returning whether both attributes
/// were accepted.
fn enable_iconic(hwnd: HWND) -> bool {
    let enabled = BOOL(1);
    let value = std::ptr::from_ref(&enabled).cast::<c_void>();
    let size = size_of::<BOOL>() as u32;
    // SAFETY: `hwnd` is the live window and `value` points at a `BOOL` that
    // outlives both calls; DWM only reads it.
    let forced =
        unsafe { DwmSetWindowAttribute(hwnd, DWMWA_FORCE_ICONIC_REPRESENTATION, value, size) };
    // SAFETY: as above.
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
