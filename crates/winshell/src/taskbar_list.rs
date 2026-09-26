//! Creation of the shell's `ITaskbarList3`, shared by the taskbar features.
//!
//! The thumbnail toolbar ([`crate::thumbbar`]) and the iconic-thumbnail +
//! progress-bar integration ([`crate::taskbar`]) each own their own
//! `ITaskbarList3` instance — they attach independently and either may fail
//! on its own — but they share the thread's COM initialisation and the
//! coclass creation here.
//!
//! This is one of the crate's raw Win32/COM surfaces: every `unsafe` block
//! carries a `// SAFETY:` comment and the public API is safe.

use std::ffi::c_void;
use std::io;

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, RPC_E_CHANGED_MODE};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::ITaskbarList3;

use crate::{Result, WinshellError};

#[cfg(windows)]
const CLSID_TASKBAR_LIST: windows::core::GUID =
    windows::core::GUID::from_u128(0x56FDF344_FD6D_11D0_958A_006097C9A090);

#[cfg(windows)]
pub(crate) fn create(hwnd: isize) -> Result<(ITaskbarList3, HWND)> {
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() && hr != RPC_E_CHANGED_MODE {
        return Err(to_io(windows::core::Error::from(hr)));
    }

    let taskbar: ITaskbarList3 =
        unsafe { CoCreateInstance(&CLSID_TASKBAR_LIST, None, CLSCTX_INPROC_SERVER) }
            .map_err(to_io)?;
    unsafe { taskbar.HrInit() }.map_err(to_io)?;

    Ok((taskbar, HWND(hwnd as *mut c_void)))
}

#[cfg(not(windows))]
pub(crate) fn create(_hwnd: isize) -> Result<()> {
    Err(WinshellError::Io(io::Error::new(
        io::ErrorKind::Unsupported,
        "Not supported on non-windows",
    )))
}

#[cfg(windows)]
pub(crate) fn to_io(err: windows::core::Error) -> WinshellError {
    WinshellError::Io(io::Error::from_raw_os_error(err.code().0))
}
