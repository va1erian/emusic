//! The entire crate's raw Win32 surface.
//!
//! Everything else in `winshell` goes through safe crates (`interprocess`
//! for named pipes, `windows-registry` for the registry). What's left here
//! has no safe wrapper anywhere: granting foreground rights to another
//! process, reading a named pipe's server PID, restoring/raising a window,
//! and telling Explorer that associations changed. Every `unsafe` block
//! carries a `// SAFETY:` comment.

use std::io;

#[cfg(windows)]
use std::os::windows::io::{AsHandle, AsRawHandle};

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::GetDriveTypeW;
#[cfg(windows)]
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
#[cfg(windows)]
use windows::Win32::System::WindowsProgramming::DRIVE_REMOTE;
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
#[cfg(windows)]
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify, ShellExecuteW};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, GetClassNameW, SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow,
    ShowWindow,
};

/// Grants the process `pid` the right to call `SetForegroundWindow`, even
/// though it isn't currently the foreground process.
#[cfg(windows)]
pub fn allow_set_foreground_window(pid: u32) -> io::Result<()> {
    // SAFETY: `AllowSetForegroundWindow` only reads `pid`; there is no
    // pointer or lifetime obligation to uphold.
    unsafe { AllowSetForegroundWindow(pid) }.map_err(|e| io::Error::from_raw_os_error(e.code().0))
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub fn allow_set_foreground_window(_pid: u32) -> io::Result<()> {
    Ok(())
}

/// Returns the process id of the server end of a connected named-pipe
/// client handle.
#[cfg(windows)]
pub fn named_pipe_server_process_id(pipe: &impl AsHandle) -> io::Result<u32> {
    let raw = pipe.as_handle().as_raw_handle();
    let handle = windows::Win32::Foundation::HANDLE(raw);
    let mut pid = 0u32;
    // SAFETY: `handle` borrows a valid, open, connected named-pipe handle
    // for the duration of this call; `pid` is a valid `u32` out-pointer.
    unsafe { GetNamedPipeServerProcessId(handle, &mut pid) }
        .map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
    Ok(pid)
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub fn named_pipe_server_process_id<T>(_pipe: &T) -> io::Result<u32> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Not supported on non-windows",
    ))
}

/// Restores (if minimized) and raises the window identified by `hwnd` (its
/// raw value) to the foreground.
#[cfg(windows)]
pub fn bring_to_front(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    // SAFETY: `ShowWindow`/`SetForegroundWindow` only require a window
    // handle; a stale or invalid `HWND` is a documented no-op/failure on
    // the Win32 side, not undefined behaviour.
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
    }
}

#[cfg(not(windows))]
pub fn bring_to_front(_hwnd: isize) {}

/// Returns whether `path` lives on a remote drive.
///
/// UNC paths (`\\server\share`) are always treated as remote. Mapped drive
/// letters are checked with `GetDriveTypeW`. Non-Windows platforms and
/// unrecognised paths return `false`.
pub fn is_remote_drive(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\") || s.starts_with(r"//") {
        return true;
    }
    let Some(first) = path.components().next() else {
        return false;
    };
    let std::path::Component::Prefix(prefix) = first else {
        return false;
    };
    match prefix.kind() {
        std::path::Prefix::VerbatimUNC(..) | std::path::Prefix::UNC(..) => true,
        #[cfg(windows)]
        std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive) => {
            let root = format!("{}:\\", drive as char);
            let root = windows::core::HSTRING::from(&*root);
            let drive_type = unsafe { GetDriveTypeW(&root) };
            drive_type == DRIVE_REMOTE
        }
        _ => false,
    }
}

/// Opens `target` — a file, folder or URI such as `ms-settings:...` — with
/// whatever handler the shell has registered for it.
#[cfg(windows)]
pub fn shell_open(target: &str) -> io::Result<()> {
    let operation = windows::core::HSTRING::from("open");
    let file = windows::core::HSTRING::from(target);
    // SAFETY: `ShellExecuteW` only reads the two valid, null-terminated
    // HSTRINGs for the duration of the call.
    let result = unsafe {
        ShellExecuteW(
            HWND::default(),
            &operation,
            &file,
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let code = result.0 as isize;
    if code <= 32 {
        Err(io::Error::from_raw_os_error(code as i32))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub fn shell_open(_target: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Not supported on non-windows",
    ))
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub fn focused_window_class() -> Option<String> {
    None
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub fn notify_assoc_changed() {}
