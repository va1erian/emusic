//! Windows taskbar thumbnail-toolbar transport buttons (#42).
//!
//! Adds previous / play-pause / next to the taskbar thumbnail preview through
//! the shell's `ITaskbarList3`, and watches for their `WM_COMMAND`
//! notifications so the app can fold them into its own command queue.
//!
//! Per the `ITaskbarList3` contract the buttons are only added in response to
//! the shell's registered `TaskbarButtonCreated` message — which it also
//! re-sends after an `explorer.exe` restart — so [`msg_hook`] watches for that
//! message as well and the app calls [`ThumbBar::add_buttons`] when
//! [`take_buttons_requested`] reports one. Adding the buttons before the
//! message arrives does not error; it silently has no effect.
//!
//! This module is one of the crate's raw Win32/COM surfaces (the rest lives
//! in `crate::sys`): every `unsafe` block carries a `// SAFETY:` comment, and
//! the public API is safe.

mod icons;

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    ITaskbarList3, THB_FLAGS, THB_ICON, THB_TOOLTIP, THBF_ENABLED, THBN_CLICKED, THUMBBUTTON,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconFromResourceEx, DestroyIcon, HICON, LR_DEFAULTCOLOR, MSG, RegisterWindowMessageW,
    WM_COMMAND,
};

use crate::Result;
#[cfg(windows)]
use crate::taskbar_list::{self, to_io};

#[cfg(windows)]
use icons::Glyph;

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

/// Cached id of the shell's registered `TaskbarButtonCreated` message.
///
/// `RegisterWindowMessageW` returns the same id for every caller, so caching
/// it in a `OnceLock` keeps the per-message check in [`msg_hook`] cheap.
static TASKBAR_BUTTON_CREATED: OnceLock<u32> = OnceLock::new();

/// Whether the shell has announced a taskbar button since the last
/// [`take_buttons_requested`], filled by [`msg_hook`].
static BUTTONS_REQUESTED: AtomicBool = AtomicBool::new(false);

/// The `TaskbarButtonCreated` message id, registering it with the shell on
/// first call.
///
/// The taskbar sends this message to a window once its taskbar button exists
/// (and again whenever `explorer.exe` restarts), which is the documented
/// trigger for [`ThumbBar::add_buttons`]; see the module docs.
#[cfg(windows)]
pub fn taskbar_button_created_message() -> u32 {
    *TASKBAR_BUTTON_CREATED.get_or_init(|| unsafe {
        RegisterWindowMessageW(windows::core::w!("TaskbarButtonCreated"))
    })
}

#[cfg(not(windows))]
pub fn taskbar_button_created_message() -> u32 {
    0
}

#[cfg(windows)]
pub fn msg_hook(msg: *const c_void) -> bool {
    let msg = unsafe { &*msg.cast::<MSG>() };
    if msg.message == taskbar_button_created_message() {
        BUTTONS_REQUESTED.store(true, Ordering::Release);
        return true;
    }
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

#[cfg(not(windows))]
pub fn msg_hook(_msg: *const c_void) -> bool {
    false
}

/// Drains the button presses recorded by [`msg_hook`].
pub fn take_clicks() -> Vec<ThumbBarButton> {
    std::mem::take(
        &mut *CLICKS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    )
}

/// Consumes the shell's pending taskbar-button announcement, if any.
///
/// Whenever this returns `true`, [`ThumbBar::add_buttons`] should be called:
/// once after startup and again if the message re-arrives because
/// `explorer.exe` restarted.
pub fn take_buttons_requested() -> bool {
    BUTTONS_REQUESTED.swap(false, Ordering::AcqRel)
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
#[cfg(windows)]
pub struct ThumbBar {
    taskbar: ITaskbarList3,
    hwnd: HWND,
    icons: Icons,
    buttons_added: bool,
}

#[cfg(not(windows))]
pub struct ThumbBar;

#[cfg(windows)]
impl ThumbBar {
    pub fn new(hwnd: isize) -> Result<Self> {
        let (taskbar, hwnd) = taskbar_list::create(hwnd)?;
        let icons = Icons::load()?;

        Ok(Self {
            taskbar,
            hwnd,
            icons,
            buttons_added: false,
        })
    }

    pub fn add_buttons(&mut self) -> Result<()> {
        let buttons = [
            button(ID_PREVIOUS, self.icons.previous, "Previous track"),
            button(ID_PLAY_PAUSE, self.icons.play, "Play"),
            button(ID_NEXT, self.icons.next, "Next track"),
        ];
        unsafe { self.taskbar.ThumbBarAddButtons(self.hwnd, &buttons) }.map_err(to_io)?;
        self.buttons_added = true;
        Ok(())
    }

    pub fn set_playing(&mut self, playing: bool) -> Result<()> {
        if !self.buttons_added {
            return Ok(());
        }
        let (icon, tooltip) = if playing {
            (self.icons.pause, "Pause")
        } else {
            (self.icons.play, "Play")
        };
        let button = button(ID_PLAY_PAUSE, icon, tooltip);
        unsafe { self.taskbar.ThumbBarUpdateButtons(self.hwnd, &[button]) }.map_err(to_io)
    }
}

#[cfg(not(windows))]
impl ThumbBar {
    pub fn new(_hwnd: isize) -> Result<Self> {
        Err(crate::WinshellError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Not supported on non-windows",
        )))
    }

    pub fn add_buttons(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn set_playing(&mut self, _playing: bool) -> Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
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
#[cfg(windows)]
struct Icons {
    previous: HICON,
    play: HICON,
    pause: HICON,
    next: HICON,
}

#[cfg(windows)]
impl Icons {
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

#[cfg(windows)]
impl Drop for Icons {
    fn drop(&mut self) {
        for icon in [self.previous, self.play, self.pause, self.next] {
            let _ = unsafe { DestroyIcon(icon) };
        }
    }
}

#[cfg(windows)]
fn load_all() -> Result<[HICON; 4]> {
    let glyphs = [Glyph::Previous, Glyph::Play, Glyph::Pause, Glyph::Next];
    let mut handles = [HICON::default(); 4];
    for (index, glyph) in glyphs.into_iter().enumerate() {
        match create_icon(glyph) {
            Ok(handle) => handles[index] = handle,
            Err(err) => {
                for handle in handles.iter().take(index) {
                    let _ = unsafe { DestroyIcon(*handle) };
                }
                return Err(err);
            }
        }
    }
    Ok(handles)
}

#[cfg(windows)]
fn create_icon(glyph: Glyph) -> Result<HICON> {
    let data = icons::resource(glyph);
    unsafe { CreateIconFromResourceEx(&data, true, 0x0003_0000, 0, 0, LR_DEFAULTCOLOR) }
        .map_err(to_io)
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

    #[test]
    fn taskbar_button_created_is_claimed_and_consumed_once() {
        let _ = take_buttons_requested();
        assert!(
            !take_buttons_requested(),
            "nothing is pending to start with"
        );

        let created = MSG {
            message: taskbar_button_created_message(),
            ..MSG::default()
        };
        assert!(dispatch(&created), "the shell's announcement is claimed");
        assert!(take_buttons_requested(), "and queued as a pending add");
        assert!(!take_buttons_requested(), "consuming clears it");
    }
}
