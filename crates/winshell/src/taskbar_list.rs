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
/// `CLSID_TaskbarList`, the shell's taskbar-list coclass. The `windows` crate
/// exposes the interface IIDs but not the class id, so it is spelled out here.
const CLSID_TASKBAR_LIST: windows::core::GUID =
    windows::core::GUID::from_u128(0x56FDF344_FD6D_11D0_958A_006097C9A090);

#[cfg(windows)]
/// Initialises COM for this thread and creates an `ITaskbarList3`.
///
/// `hwnd` is the raw window handle the returned object is bound to. Returns
/// an error when COM or the shell object cannot be created — e.g. Explorer is
/// not running — so a caller can log it and carry on without its feature
/// rather than crash.
pub(crate) fn create(hwnd: isize) -> Result<(ITaskbarList3, HWND)> {
    // SAFETY: `CoInitializeEx` only initialises the calling thread's COM
    // apartment and dereferences no pointer (we pass `None`). This lives as
    // long as the UI thread and is deliberately never balanced with
    // `CoUninitialize`, so we never tear down an apartment another module set
    // up (a repeat call returns `S_FALSE`/`RPC_E_CHANGED_MODE`, both fine).
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() && hr != RPC_E_CHANGED_MODE {
        return Err(to_io(windows::core::Error::from(hr)));
    }

    // SAFETY: `CLSID_TaskbarList` is registered by the shell; an in-proc
    // server with no outer unknown is the documented way to create it. The
    // returned interface is fully owned by the caller.
    let taskbar: ITaskbarList3 =
        unsafe { CoCreateInstance(&CLSID_TASKBAR_LIST, None, CLSCTX_INPROC_SERVER) }
            .map_err(to_io)?;
    // SAFETY: `HrInit` is required before any other `ITaskbarList` call and
    // touches no Rust memory.
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
/// Wraps a Win32 error as the crate's I/O error, as `crate::sys` does.
pub(crate) fn to_io(err: windows::core::Error) -> WinshellError {
    WinshellError::Io(io::Error::from_raw_os_error(err.code().0))
}
