//! The entire crate's raw Win32 surface.
//!
//! Everything else in `winshell` goes through safe crates (`interprocess`
//! for named pipes, `windows-registry` for the registry). What's left here
//! has no safe wrapper anywhere: granting foreground rights to another
//! process, reading a named pipe's server PID, restoring/raising a window,
//! and telling Explorer that associations changed. Every `unsafe` block
//! carries a `// SAFETY:` comment.

use std::io;
use std::os::windows::io::{AsHandle, AsRawHandle};

use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::FileSystem::GetDriveTypeW;
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows::Win32::System::WindowsProgramming::DRIVE_REMOTE;
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, GetClassNameW, SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow,
    ShowWindow,
};

/// Grants the process `pid` the right to call `SetForegroundWindow`, even
/// though it isn't currently the foreground process.
///
/// Used by the primary instance right after accepting an IPC connection, so
/// the (briefly foreground) secondary launcher can hand control back to it.
pub fn allow_set_foreground_window(pid: u32) -> io::Result<()> {
    // SAFETY: `AllowSetForegroundWindow` only reads `pid`; there is no
    // pointer or lifetime obligation to uphold.
    unsafe { AllowSetForegroundWindow(pid) }.map_err(|e| io::Error::from_raw_os_error(e.code().0))
}

/// Returns the process id of the server end of a connected named-pipe
/// client handle (anything implementing [`AsHandle`], e.g. an
/// `interprocess` `PipeStream`).
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

/// Restores (if minimized) and raises the window identified by `hwnd` (its
/// raw value) to the foreground.
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

/// Returns whether `path` lives on a remote drive.
///
/// UNC paths (`\\server\share`) are always treated as remote. Mapped drive
/// letters are checked with `GetDriveTypeW`. Non-Windows platforms and
/// unrecognised paths return `false`.
pub fn is_remote_drive(path: &std::path::Path) -> bool {
    let Some(first) = path.components().next() else {
        return false;
    };
    let std::path::Component::Prefix(prefix) = first else {
        return false;
    };
    match prefix.kind() {
        std::path::Prefix::VerbatimUNC(..) | std::path::Prefix::UNC(..) => true,
        std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive) => {
            let root = format!("{}:\\", drive as char);
            let root = windows::core::HSTRING::from(&*root);
            // SAFETY: `GetDriveTypeW` only reads its argument; `root` is a
            // valid HSTRING that outlives the call.
            let drive_type = unsafe { GetDriveTypeW(&root) };
            drive_type == DRIVE_REMOTE
        }
        _ => false,
    }
}

/// Opens `target` — a file, folder or URI such as `ms-settings:...` — with
/// whatever handler the shell has registered for it, exactly as double-
/// clicking it in Explorer would.
///
/// `explorer.exe` cannot launch a `ms-settings:` URI: it treats the argument
/// as a filesystem path, fails to resolve it and opens Documents instead. The
/// URI therefore has to go through `ShellExecuteW`.
pub fn shell_open(target: &str) -> io::Result<()> {
    let operation = windows::core::HSTRING::from("open");
    let file = windows::core::HSTRING::from(target);
    // SAFETY: `ShellExecuteW` only reads the two valid, null-terminated
    // HSTRINGs for the duration of the call; a null `hwnd` is the documented
    // way to open a URI without an owning window. The returned `HINSTANCE` is
    // not an owned handle for this call — only its value matters, and any
    // value <= 32 is a documented error code rather than a handle.
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

/// The window class name of the window that currently has keyboard focus, if
/// any.
///
/// Returns `None` when no window has focus (e.g. the app is not foreground) or
/// the class name cannot be read. The class name is the raw Win32 name — e.g.
/// `"Edit"` for a text field — not a friendly label.
pub fn focused_window_class() -> Option<String> {
    // SAFETY: `GetFocus` takes no arguments and only reads the calling
    // thread's focus window; a null result is its documented "no focus" case.
    let hwnd = unsafe { GetFocus() };
    if hwnd.0.is_null() {
        return None;
    }
    let mut buffer = [0u16; 256];
    // SAFETY: `buffer` is a valid, writable slice of `buffer.len()` UTF-16
    // code units, and `hwnd` is a live window handle for the duration of the
    // call; the returned length is at most `buffer.len()`.
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

/// Tells Explorer that file associations changed, so icons and "Open with"
/// menus refresh without a logoff/logon.
pub fn notify_assoc_changed() {
    // SAFETY: `SHChangeNotify` with `SHCNE_ASSOCCHANGED`/`SHCNF_IDLIST`
    // ignores `dwItem1`/`dwItem2`; passing `None` for both is the
    // documented usage for a global association-change notification.
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
}
