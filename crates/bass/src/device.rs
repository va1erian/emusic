//! Output device enumeration (`BASS_GetDeviceInfo`).

use std::ffi::CStr;

use crate::error::BassError;
use crate::ffi::{BassLib, consts as c, types::BassDeviceInfo};

/// Information about one output device, as reported by `BASS_GetDeviceInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Zero-based device index (pass to [`crate::Bass::init`]).
    pub index: u32,
    /// Device description, e.g. `"Speakers (Realtek Audio)"`.
    pub name: String,
    /// Driver name/identifier.
    pub driver: String,
    /// Whether the device is enabled.
    pub enabled: bool,
    /// Whether this is the system's default device.
    pub is_default: bool,
    /// Whether the device is already initialized (via `BASS_Init`).
    pub initialized: bool,
}

/// Enumerates every output device BASS can see, stopping at the first index
/// for which `BASS_GetDeviceInfo` fails (that's how BASS signals the end of
/// the list).
pub(crate) fn enumerate(lib: &BassLib) -> Result<Vec<DeviceInfo>, BassError> {
    let mut devices = Vec::new();
    let mut index = 0u32;
    loop {
        let mut info = BassDeviceInfo::default();
        // SAFETY: `info` is a valid, writable `BASS_DEVICEINFO` for the
        // duration of this call; BASS fills it in or leaves it untouched
        // and returns `FALSE` if `index` is out of range.
        let ok = unsafe { (lib.raw.bass_get_device_info)(index, &mut info) } != 0;
        if !ok {
            break;
        }
        devices.push(device_info_from_raw(index, &info));
        index += 1;
    }
    Ok(devices)
}

fn device_info_from_raw(index: u32, info: &BassDeviceInfo) -> DeviceInfo {
    // SAFETY: BASS returns pointers to static/internal strings that stay
    // valid at least until the next `BASS_GetDeviceInfo` call, which is
    // long enough for us to copy them out here.
    let name = unsafe { cstr_to_string(info.name) };
    // SAFETY: see above.
    let driver = unsafe { cstr_to_string(info.driver) };
    DeviceInfo {
        index,
        name,
        driver,
        enabled: info.flags & c::BASS_DEVICE_ENABLED != 0,
        is_default: info.flags & c::BASS_DEVICE_DEFAULT != 0,
        initialized: info.flags & c::BASS_DEVICE_INIT != 0,
    }
}

/// # Safety
/// `ptr` must be either null or point to a valid, NUL-terminated C string
/// that stays valid for the duration of this call.
unsafe fn cstr_to_string(ptr: *const std::os::raw::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: caller guarantees `ptr` is a valid NUL-terminated C string.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_decode_into_booleans() {
        let raw = BassDeviceInfo {
            name: std::ptr::null(),
            driver: std::ptr::null(),
            flags: c::BASS_DEVICE_ENABLED | c::BASS_DEVICE_DEFAULT,
        };
        let info = device_info_from_raw(2, &raw);
        assert_eq!(info.index, 2);
        assert!(info.enabled);
        assert!(info.is_default);
        assert!(!info.initialized);
        assert_eq!(info.name, "");
    }
}
