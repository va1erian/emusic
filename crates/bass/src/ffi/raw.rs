//! The table of raw BASS function pointers, resolved once at load time.

use std::ffi::c_void;

use libloading::Library;

use super::types::{
    BassChannelInfo, BassDeviceInfo, BassMidiFont, Bool, Dword, HMusic, HPlugin, HSoundFont,
    HStream, HSync, Qword, StreamProc, SyncProc,
};
use crate::error::BassError;

/// All BASS entry points the safe API needs, resolved once from the loaded
/// `bass.dll` and kept alongside it.
///
/// Every field is a raw `extern "system"` function pointer with the exact
/// signature BASS exports; nothing here does bounds/validity checking,
/// that's the job of the safe wrappers in `crate::{stream,music,channel,..}`.
pub(crate) struct RawBindings {
    pub bass_init: unsafe extern "system" fn(
        device: i32,
        freq: Dword,
        flags: Dword,
        win: *mut c_void,
        clsid: *const c_void,
    ) -> Bool,
    pub bass_free: unsafe extern "system" fn() -> Bool,
    pub bass_error_get_code: unsafe extern "system" fn() -> i32,
    pub bass_get_version: unsafe extern "system" fn() -> Dword,
    pub bass_set_config: unsafe extern "system" fn(option: Dword, value: Dword) -> Bool,
    pub bass_get_config: unsafe extern "system" fn(option: Dword) -> Dword,
    pub bass_set_config_ptr: unsafe extern "system" fn(option: Dword, value: *const c_void) -> Bool,
    pub bass_get_device_info:
        unsafe extern "system" fn(device: Dword, info: *mut BassDeviceInfo) -> Bool,
    pub bass_plugin_load: unsafe extern "system" fn(file: *const c_void, flags: Dword) -> HPlugin,
    pub bass_stream_create_file: unsafe extern "system" fn(
        mem: Bool,
        file: *const c_void,
        offset: Qword,
        length: Qword,
        flags: Dword,
    ) -> HStream,
    pub bass_stream_create: unsafe extern "system" fn(
        freq: Dword,
        chans: Dword,
        flags: Dword,
        proc_: *const StreamProc,
        user: *mut c_void,
    ) -> HStream,
    pub bass_stream_put_data:
        unsafe extern "system" fn(handle: HStream, buffer: *const c_void, length: Dword) -> Dword,
    pub bass_stream_free: unsafe extern "system" fn(handle: HStream) -> Bool,
    pub bass_music_load: unsafe extern "system" fn(
        mem: Bool,
        file: *const c_void,
        offset: Qword,
        length: Dword,
        flags: Dword,
        freq: Dword,
    ) -> HMusic,
    pub bass_music_free: unsafe extern "system" fn(handle: HMusic) -> Bool,
    pub bass_channel_play: unsafe extern "system" fn(handle: Dword, restart: Bool) -> Bool,
    pub bass_channel_pause: unsafe extern "system" fn(handle: Dword) -> Bool,
    pub bass_channel_stop: unsafe extern "system" fn(handle: Dword) -> Bool,
    pub bass_channel_is_active: unsafe extern "system" fn(handle: Dword) -> Dword,
    pub bass_channel_get_length: unsafe extern "system" fn(handle: Dword, mode: Dword) -> Qword,
    pub bass_channel_get_position: unsafe extern "system" fn(handle: Dword, mode: Dword) -> Qword,
    pub bass_channel_set_position:
        unsafe extern "system" fn(handle: Dword, pos: Qword, mode: Dword) -> Bool,
    pub bass_channel_bytes_2_seconds: unsafe extern "system" fn(handle: Dword, pos: Qword) -> f64,
    pub bass_channel_seconds_2_bytes: unsafe extern "system" fn(handle: Dword, pos: f64) -> Qword,
    pub bass_channel_get_attribute:
        unsafe extern "system" fn(handle: Dword, attrib: Dword, value: *mut f32) -> Bool,
    pub bass_channel_set_attribute:
        unsafe extern "system" fn(handle: Dword, attrib: Dword, value: f32) -> Bool,
    pub bass_channel_flags:
        unsafe extern "system" fn(handle: Dword, flags: Dword, mask: Dword) -> Dword,
    pub bass_channel_get_data:
        unsafe extern "system" fn(handle: Dword, buffer: *mut c_void, length: Dword) -> i32,
    pub bass_channel_get_info:
        unsafe extern "system" fn(handle: Dword, info: *mut BassChannelInfo) -> Bool,
    pub bass_channel_set_sync: unsafe extern "system" fn(
        handle: Dword,
        kind: Dword,
        param: Qword,
        proc_: SyncProc,
        user: *mut c_void,
    ) -> HSync,
    pub bass_channel_remove_sync: unsafe extern "system" fn(handle: Dword, sync: HSync) -> Bool,
    pub bass_channel_get_tags:
        unsafe extern "system" fn(handle: Dword, tags: Dword) -> *const c_void,
}

/// `bassmidi.dll`'s own exports, resolved separately from `bass.dll`'s
/// [`RawBindings`] (see [`super::MidiLib`]): needed to change an
/// already-open MIDI channel's soundfont live, which
/// `BASS_CONFIG_MIDI_DEFFONT` (a `bass.dll` config option) can't do.
pub(crate) struct MidiRawBindings {
    pub bass_midi_font_init:
        unsafe extern "system" fn(file: *const c_void, flags: Dword) -> HSoundFont,
    pub bass_midi_font_free: unsafe extern "system" fn(handle: HSoundFont) -> Bool,
    pub bass_midi_stream_set_fonts:
        unsafe extern "system" fn(handle: Dword, fonts: *const BassMidiFont, count: Dword) -> Dword,
}

impl MidiRawBindings {
    /// Resolves every symbol this crate needs out of an already-loaded
    /// `bassmidi.dll`.
    pub(crate) fn load(lib: &Library) -> Result<Self, BassError> {
        Ok(Self {
            // SAFETY: transcribed from the documented BASSMIDI 2.4 C API.
            bass_midi_font_init: unsafe { symbol(lib, "BASS_MIDI_FontInit")? },
            // SAFETY: see above.
            bass_midi_font_free: unsafe { symbol(lib, "BASS_MIDI_FontFree")? },
            // SAFETY: see above.
            bass_midi_stream_set_fonts: unsafe { symbol(lib, "BASS_MIDI_StreamSetFonts")? },
        })
    }
}

/// Loads one symbol from `lib` by its C name, mapping a lookup failure to
/// [`BassError::SymbolNotFound`].
///
/// # Safety
/// The caller must ensure `T` exactly matches the calling convention and
/// signature of the symbol named `name` as exported by `bass.dll`; getting
/// this wrong is undefined behaviour the moment the pointer is called.
unsafe fn symbol<T: Copy>(lib: &Library, name: &str) -> Result<T, BassError> {
    // SAFETY: caller guarantees `T` matches `name`'s real signature. The
    // null-terminated byte string is only used for the duration of the
    // lookup.
    unsafe {
        lib.get::<T>(name.as_bytes())
            .map(|sym| *sym)
            .map_err(|_| BassError::SymbolNotFound(name.to_string()))
    }
}

macro_rules! load_symbols {
    ($lib:expr, { $($field:ident : $name:literal),+ $(,)? }) => {
        RawBindings {
            $(
                // SAFETY: each signature above is transcribed from the
                // documented BASS 2.4 C API for the matching exported name.
                $field: unsafe { symbol($lib, $name)? },
            )+
        }
    };
}

impl RawBindings {
    /// Resolves every symbol this crate needs out of an already-loaded
    /// `bass.dll`.
    pub(crate) fn load(lib: &Library) -> Result<Self, BassError> {
        Ok(load_symbols!(lib, {
            bass_init: "BASS_Init",
            bass_free: "BASS_Free",
            bass_error_get_code: "BASS_ErrorGetCode",
            bass_get_version: "BASS_GetVersion",
            bass_set_config: "BASS_SetConfig",
            bass_get_config: "BASS_GetConfig",
            bass_set_config_ptr: "BASS_SetConfigPtr",
            bass_get_device_info: "BASS_GetDeviceInfo",
            bass_plugin_load: "BASS_PluginLoad",
            bass_stream_create_file: "BASS_StreamCreateFile",
            bass_stream_create: "BASS_StreamCreate",
            bass_stream_put_data: "BASS_StreamPutData",
            bass_stream_free: "BASS_StreamFree",
            bass_music_load: "BASS_MusicLoad",
            bass_music_free: "BASS_MusicFree",
            bass_channel_play: "BASS_ChannelPlay",
            bass_channel_pause: "BASS_ChannelPause",
            bass_channel_stop: "BASS_ChannelStop",
            bass_channel_is_active: "BASS_ChannelIsActive",
            bass_channel_get_length: "BASS_ChannelGetLength",
            bass_channel_get_position: "BASS_ChannelGetPosition",
            bass_channel_set_position: "BASS_ChannelSetPosition",
            bass_channel_bytes_2_seconds: "BASS_ChannelBytes2Seconds",
            bass_channel_seconds_2_bytes: "BASS_ChannelSeconds2Bytes",
            bass_channel_get_attribute: "BASS_ChannelGetAttribute",
            bass_channel_set_attribute: "BASS_ChannelSetAttribute",
            bass_channel_flags: "BASS_ChannelFlags",
            bass_channel_get_data: "BASS_ChannelGetData",
            bass_channel_get_info: "BASS_ChannelGetInfo",
            bass_channel_set_sync: "BASS_ChannelSetSync",
            bass_channel_remove_sync: "BASS_ChannelRemoveSync",
            bass_channel_get_tags: "BASS_ChannelGetTags",
        }))
    }
}
