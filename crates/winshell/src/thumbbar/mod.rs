//! Windows taskbar thumbnail-toolbar transport buttons (#42).
//!
//! Adds previous / play-pause / next to the taskbar thumbnail preview through
//! the shell's `ITaskbarList3`, and watches for their `WM_COMMAND`
//! notifications so the app can fold them into its own command queue.
//!
//! This module is one of the crate's raw Win32/COM surfaces (the rest lives
//! in `crate::sys`): every `unsafe` block carries a `// SAFETY:` comment, and
//! the public API is safe.

mod icons;

use std::ffi::c_void;
use std::io;
use std::sync::Mutex;

use windows::Win32::Foundation::{HWND, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::Shell::{
    ITaskbarList3, THB_FLAGS, THB_ICON, THB_TOOLTIP, THBF_ENABLED, THBN_CLICKED, THUMBBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, DestroyIcon, HICON, LR_DEFAULTCOLOR, MSG, WM_COMMAND,
};

use crate::{Result, WinshellError};

use icons::Glyph;

/// `CLSID_TaskbarList`, the shell's taskbar-list coclass. The `windows` crate
/// exposes the interface IIDs but not the class id, so it is spelled out here.
const CLSID_TASKBAR_LIST: windows::core::GUID =
    windows::core::GUID::from_u128(0x56FDF344_FD6D_11D0_958A_006097C9A090);

/// Control id of the previous-track button. The ids are high enough that they
/// cannot clash with the app's own menu/control ids.
const ID_PREVIOUS: u32 = 0xE001;
/// Control id of the play/pause button.
const ID_PLAY_PAUSE: u32 = 0xE002;
/// Control id of the next-track button.
const ID_NEXT: u32 = 0xE003;

/// A transport action requested by a taskbar thumbnail-toolbar button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbBarButton {
    Previous,
    PlayPause,
    Next,
}

/// The button presses seen since the last [`take_clicks`], filled by
/// [`msg_hook`].
///
/// Global because the hook is installed on winit's event loop as a `'static`
/// `FnMut` and so cannot borrow the app; there is only one main window per
/// process, so one queue is enough. A poisoned lock only means a previous
/// hook panicked while holding it — the queue itself is still consistent, so
/// it is recovered rather than propagated.
static CLICKS: Mutex<Vec<ThumbBarButton>> = Mutex::new(Vec::new());

/// winit message hook turning the window's thumbnail-toolbar `WM_COMMAND`s
/// into queued [`ThumbBarButton`]s.
///
/// Intended solely as the callback passed to
/// `winit::platform::windows::EventLoopBuilderExtWindows::with_msg_hook`,
/// which guarantees `msg` points to a live `MSG` for the duration of the
/// call; that contract is why this can stay a safe function despite taking a
/// raw pointer. Returns `true` for the messages it claims (so winit skips
/// dispatching them) and `false` for everything else.
pub fn msg_hook(msg: *const c_void) -> bool {
    // SAFETY: `msg` is winit's pointer to the `MSG` it is about to dispatch
    // (and so is aligned and valid for this call). Our `MSG` and winit's
    // `windows-sys` one are both `#[repr(C)]` with identical fields, so the
    // reinterpretation is layout-compatible. We only read it.
    let msg = unsafe { &*msg.cast::<MSG>() };
    if msg.message != WM_COMMAND {
        return false;
    }
    let id = (msg.wParam.0 & 0xffff) as u32;
    let notification = ((msg.wParam.0 >> 16) & 0xffff) as u32;
    if notification != THBN_CLICKED {
        return false;
    }
    let Some(button) = button_for_id(id) else {
        return false;
    };
    CLICKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(button);
    true
}

/// Drains the button presses recorded by [`msg_hook`].
pub fn take_clicks() -> Vec<ThumbBarButton> {
    std::mem::take(
        &mut *CLICKS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    )
}

/// Maps a `WM_COMMAND` control id to the button it identifies.
fn button_for_id(id: u32) -> Option<ThumbBarButton> {
    match id {
        ID_PREVIOUS => Some(ThumbBarButton::Previous),
        ID_PLAY_PAUSE => Some(ThumbBarButton::PlayPause),
        ID_NEXT => Some(ThumbBarButton::Next),
        _ => None,
    }
}

/// The shell's `ITaskbarList3`, plus the icons it displays.
///
/// Lives on the UI thread for as long as the app: the COM interface is not
/// `Send` and the shell keeps using the buttons until the window goes away.
pub struct ThumbBar {
    taskbar: ITaskbarList3,
    hwnd: HWND,
    icons: Icons,
}

impl ThumbBar {
    /// Adds the previous / play-pause / next buttons to `hwnd`'s taskbar
    /// thumbnail preview.
    ///
    /// `hwnd` is the raw value of the window handle. Returns an error when
    /// COM, the shell object or the icons cannot be created — e.g. Explorer
    /// is not running — so the caller can log it and carry on without the
    /// buttons rather than crash.
    pub fn new(hwnd: isize) -> Result<Self> {
        // SAFETY: `CoInitializeEx` only initialises the calling thread's COM
        // apartment and dereferences no pointer (we pass `None`). This lives
        // as long as the UI thread and is deliberately never balanced with
        // `CoUninitialize`, so we never tear down an apartment winit set up.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            return Err(to_io(windows::core::Error::from(hr)));
        }

        // SAFETY: `CLSID_TaskbarList` is registered by the shell; an in-proc
        // server with no outer unknown is the documented way to create it.
        // The returned interface is fully owned by us.
        let taskbar: ITaskbarList3 =
            unsafe { CoCreateInstance(&CLSID_TASKBAR_LIST, None, CLSCTX_INPROC_SERVER) }
                .map_err(to_io)?;
        // SAFETY: `HrInit` is required before any other `ITaskbarList` call
        // and touches no Rust memory.
        unsafe { taskbar.HrInit() }.map_err(to_io)?;

        let hwnd = HWND(hwnd as *mut c_void);
        let icons = Icons::load()?;
        let buttons = [
            button(ID_PREVIOUS, icons.previous, "Previous track"),
            button(ID_PLAY_PAUSE, icons.play, "Play"),
            button(ID_NEXT, icons.next, "Next track"),
        ];
        // SAFETY: `hwnd` is the app's live top-level window and `buttons` is
        // a valid slice of fully-initialised `THUMBBUTTON`s whose icons
        // outlive this call (owned by `icons`, moved into `self` below).
        unsafe { taskbar.ThumbBarAddButtons(hwnd, &buttons) }.map_err(to_io)?;

        Ok(Self {
            taskbar,
            hwnd,
            icons,
        })
    }

    /// Swaps the play/pause button's glyph and tooltip to match the player.
    pub fn set_playing(&mut self, playing: bool) -> Result<()> {
        let (icon, tooltip) = if playing {
            (self.icons.pause, "Pause")
        } else {
            (self.icons.play, "Play")
        };
        let button = button(ID_PLAY_PAUSE, icon, tooltip);
        // SAFETY: `hwnd` is still the window the buttons were added to, and
        // `button` is a fully-initialised `THUMBBUTTON` whose icon is owned
        // by `self.icons`.
        unsafe { self.taskbar.ThumbBarUpdateButtons(self.hwnd, &[button]) }.map_err(to_io)
    }
}

/// Builds one enabled `THUMBBUTTON` with an icon and a tooltip.
fn button(id: u32, icon: HICON, tooltip: &str) -> THUMBBUTTON {
    let mut sz_tip = [0u16; 260];
    for (slot, unit) in sz_tip.iter_mut().zip(tooltip.encode_utf16()) {
        *slot = unit;
    }
    THUMBBUTTON {
        dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
        iId: id,
        iBitmap: 0,
        hIcon: icon,
        szTip: sz_tip,
        dwFlags: THBF_ENABLED,
    }
}

/// The four glyphs, kept alive for as long as the buttons: the taskbar does
/// not take ownership of the `HICON`s it is handed.
struct Icons {
    previous: HICON,
    play: HICON,
    pause: HICON,
    next: HICON,
}

impl Icons {
    /// Rasterises and registers all four icons, or fails without leaking any
    /// that were already created.
    fn load() -> Result<Self> {
        let [previous, play, pause, next] = load_all()?;
        Ok(Self {
            previous,
            play,
            pause,
            next,
        })
    }
}

impl Drop for Icons {
    fn drop(&mut self) {
        for icon in [self.previous, self.play, self.pause, self.next] {
            // SAFETY: each handle came from `CreateIconFromResourceEx` and is
            // owned only by this struct; destroying it once is correct.
            let _ = unsafe { DestroyIcon(icon) };
        }
    }
}

/// Creates the four `HICON`s, destroying the ones already made if a later one
/// fails so a partial result never leaks.
fn load_all() -> Result<[HICON; 4]> {
    let glyphs = [Glyph::Previous, Glyph::Play, Glyph::Pause, Glyph::Next];
    let mut handles = [HICON::default(); 4];
    for (index, glyph) in glyphs.into_iter().enumerate() {
        match create_icon(glyph) {
            Ok(handle) => handles[index] = handle,
            Err(err) => {
                for handle in handles.iter().take(index) {
                    // SAFETY: these handles were created above and are not
                    // used again after this loop returns.
                    let _ = unsafe { DestroyIcon(*handle) };
                }
                return Err(err);
            }
        }
    }
    Ok(handles)
}

/// Turns one glyph into an `HICON`.
fn create_icon(glyph: Glyph) -> Result<HICON> {
    let data = icons::resource(glyph);
    // SAFETY: `data` is a well-formed 32bpp icon resource built by `icons`;
    // `ficon = true` and version `0x0003_0000` describe it exactly, and the
    // zero sizes ask Windows to use the resource's own dimensions. The
    // returned handle is owned by the caller.
    unsafe { CreateIconFromResourceEx(&data, true, 0x0003_0000, 0, 0, LR_DEFAULTCOLOR) }
        .map_err(to_io)
}

/// Wraps a Win32 error as the crate's I/O error, as `crate::sys` does.
fn to_io(err: windows::core::Error) -> WinshellError {
    WinshellError::Io(io::Error::from_raw_os_error(err.code().0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_msg(id: u32, notification: u32) -> MSG {
        MSG {
            message: WM_COMMAND,
            wParam: windows::Win32::Foundation::WPARAM(
                ((notification as usize) << 16) | id as usize,
            ),
            ..MSG::default()
        }
    }

    fn dispatch(msg: &MSG) -> bool {
        msg_hook((msg as *const MSG).cast::<c_void>())
    }

    #[test]
    fn claimed_clicks_are_queued_and_unrelated_messages_are_ignored() {
        let _ = take_clicks();

        assert!(!dispatch(&MSG::default()), "a non-WM_COMMAND is ignored");
        let wrong_notification = command_msg(ID_PLAY_PAUSE, 0);
        assert!(!dispatch(&wrong_notification));
        let unknown_id = command_msg(0x1234, THBN_CLICKED);
        assert!(!dispatch(&unknown_id));

        assert!(dispatch(&command_msg(ID_PREVIOUS, THBN_CLICKED)));
        assert!(dispatch(&command_msg(ID_PLAY_PAUSE, THBN_CLICKED)));

        assert_eq!(
            take_clicks(),
            vec![ThumbBarButton::Previous, ThumbBarButton::PlayPause]
        );
        assert!(take_clicks().is_empty(), "draining clears the queue");
    }
}
