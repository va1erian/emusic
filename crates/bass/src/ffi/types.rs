//! Raw C types and struct layouts mirroring `bass.h` (BASS 2.4).
//!
//! Nothing in this module is safe to use on its own; it only exists to give
//! [`super::raw`] accurate signatures to load symbols against.

use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

/// `DWORD` — BASS's 32-bit unsigned handle/flag type.
pub type Dword = u32;
/// `QWORD` — BASS's 64-bit unsigned type used for byte positions.
pub type Qword = u64;
/// `BOOL` as used by BASS (`0` = false, nonzero = true).
pub type Bool = c_int;
/// `HSTREAM` — handle to a sample stream.
pub type HStream = Dword;
/// `HMUSIC` — handle to a MOD music.
pub type HMusic = Dword;
/// `HSYNC` — handle to a synchronizer.
pub type HSync = Dword;
/// `HPLUGIN` — handle to a loaded add-on.
pub type HPlugin = Dword;

/// Callback signature for `BASS_SyncProc`.
///
/// BASS invokes this on an internal thread, so the wrapper never calls user
/// code directly here — see [`crate::sync`].
pub type SyncProc =
    unsafe extern "system" fn(handle: HSync, channel: Dword, data: Dword, user: *mut c_void);

/// Callback signature for `STREAMPROC` (`BASS_StreamCreate`).
///
/// A push stream passes [`STREAMPROC_PUSH`](super::consts::STREAMPROC_PUSH)
/// instead of a real callback, so this type only exists to give
/// `BASS_StreamCreate` an accurate signature.
pub type StreamProc = unsafe extern "system" fn(
    handle: HStream,
    buffer: *mut c_void,
    length: Dword,
    user: *mut c_void,
) -> Dword;

/// Mirrors `BASS_DEVICEINFO`. The `name`/`driver` pointers are only valid
/// for the lifetime of the `BASS_GetDeviceInfo` call that filled them in.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BassDeviceInfo {
    pub name: *const c_char,
    pub driver: *const c_char,
    pub flags: Dword,
}

/// Mirrors `BASS_CHANNELINFO`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BassChannelInfo {
    pub freq: Dword,
    pub chans: Dword,
    pub flags: Dword,
    pub ctype: Dword,
    pub origres: Dword,
    pub plugin: HPlugin,
    pub sample: Dword,
    pub filename: *const c_char,
}

impl Default for BassDeviceInfo {
    fn default() -> Self {
        Self {
            name: std::ptr::null(),
            driver: std::ptr::null(),
            flags: 0,
        }
    }
}
