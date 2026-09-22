//! Raw numeric constants from the BASS 2.4 C header (`bass.h`).
//!
//! These are transcribed from the published BASS 2.4 API documentation and
//! from memory of the header; they are **not** re-downloaded or copied from
//! `bass.h` itself (the header is un4seen's proprietary SDK and isn't
//! fetched by this crate). Double-check the numeric values here against the
//! real `bass.h` that ships with the DLL before shipping a release build —
//! see the crate-level docs and the PR description for details.
//!
//! Only constants actually used by the safe API are defined; the full
//! header has many more (video/3D/recording/net-streaming) that this crate
//! doesn't need yet.

#![allow(dead_code)]

use super::types::Dword;

// ---- BASS_ErrorGetCode -----------------------------------------------
pub const BASS_OK: i32 = 0;
pub const BASS_ERROR_MEM: i32 = 1;
pub const BASS_ERROR_FILEOPEN: i32 = 2;
pub const BASS_ERROR_DRIVER: i32 = 3;
pub const BASS_ERROR_BUFLOST: i32 = 4;
pub const BASS_ERROR_HANDLE: i32 = 5;
pub const BASS_ERROR_FORMAT: i32 = 6;
pub const BASS_ERROR_POSITION: i32 = 7;
pub const BASS_ERROR_INIT: i32 = 8;
pub const BASS_ERROR_START: i32 = 9;
pub const BASS_ERROR_SSL: i32 = 10;
pub const BASS_ERROR_REINIT: i32 = 11;
pub const BASS_ERROR_ALREADY: i32 = 14;
pub const BASS_ERROR_NOTAUDIO: i32 = 17;
pub const BASS_ERROR_NOCHAN: i32 = 18;
pub const BASS_ERROR_ILLTYPE: i32 = 19;
pub const BASS_ERROR_ILLPARAM: i32 = 20;
pub const BASS_ERROR_NO3D: i32 = 21;
pub const BASS_ERROR_NOEAX: i32 = 22;
pub const BASS_ERROR_DEVICE: i32 = 23;
pub const BASS_ERROR_NOPLAY: i32 = 24;
pub const BASS_ERROR_FREQ: i32 = 25;
pub const BASS_ERROR_NOTFILE: i32 = 27;
pub const BASS_ERROR_NOHW: i32 = 29;
pub const BASS_ERROR_EMPTY: i32 = 31;
pub const BASS_ERROR_NONET: i32 = 32;
pub const BASS_ERROR_CREATE: i32 = 33;
pub const BASS_ERROR_NOFX: i32 = 34;
pub const BASS_ERROR_NOTAVAIL: i32 = 37;
pub const BASS_ERROR_DECODE: i32 = 38;
pub const BASS_ERROR_DX: i32 = 39;
pub const BASS_ERROR_TIMEOUT: i32 = 40;
pub const BASS_ERROR_FILEFORM: i32 = 41;
pub const BASS_ERROR_SPEAKER: i32 = 42;
pub const BASS_ERROR_VERSION: i32 = 43;
pub const BASS_ERROR_CODEC: i32 = 44;
pub const BASS_ERROR_ENDED: i32 = 45;
pub const BASS_ERROR_BUSY: i32 = 46;
pub const BASS_ERROR_UNSTREAMABLE: i32 = 47;
pub const BASS_ERROR_PROTOCOL: i32 = 48;
pub const BASS_ERROR_DENIED: i32 = 49;
pub const BASS_ERROR_UNKNOWN: i32 = -1;

// ---- General flags ------------------------------------------------------
/// Applies to any BASS function taking a filename: the filename pointer is
/// a UTF-16 (`wchar_t`) string rather than ANSI/UTF-8.
pub const BASS_UNICODE: Dword = 0x8000_0000;

// ---- BASS_GetDeviceInfo flags -------------------------------------------
pub const BASS_DEVICE_ENABLED: Dword = 1;
pub const BASS_DEVICE_DEFAULT: Dword = 2;
pub const BASS_DEVICE_INIT: Dword = 4;

// ---- BASS_SetConfig options ----------------------------------------------
pub const BASS_CONFIG_BUFFER: Dword = 0;
pub const BASS_CONFIG_UPDATEPERIOD: Dword = 1;
pub const BASS_CONFIG_GVOL_SAMPLE: Dword = 4;
pub const BASS_CONFIG_GVOL_STREAM: Dword = 5;
pub const BASS_CONFIG_GVOL_MUSIC: Dword = 6;
pub const BASS_CONFIG_PAUSE_NOPLAY: Dword = 13;
pub const BASS_CONFIG_SRC: Dword = 43;
pub const BASS_CONFIG_SRC_SAMPLE: Dword = 44;
pub const BASS_CONFIG_FLOAT: Dword = 54;

// ---- BASS_StreamCreateFile / BASS_MusicLoad shared flags -----------------
pub const BASS_SAMPLE_FLOAT: Dword = 256;
pub const BASS_SAMPLE_MONO: Dword = 2;
pub const BASS_SAMPLE_LOOP: Dword = 4;
pub const BASS_STREAM_PRESCAN: Dword = 0x0002_0000;
pub const BASS_STREAM_AUTOFREE: Dword = 0x0004_0000;
pub const BASS_STREAM_DECODE: Dword = 0x0020_0000;

// ---- BASS_MusicLoad-specific flags ---------------------------------------
pub const BASS_MUSIC_LOOP: Dword = BASS_SAMPLE_LOOP;
pub const BASS_MUSIC_RAMP: Dword = 0x0000_0200;
pub const BASS_MUSIC_RAMPS: Dword = 0x0000_0400;
pub const BASS_MUSIC_SURROUND: Dword = 0x0000_0800;
pub const BASS_MUSIC_SURROUND2: Dword = 0x0000_1000;
/// `BASS_MUSIC_FT2PAN` and `BASS_MUSIC_FT2MOD` are aliases for the same bit
/// in `bass.h`.
pub const BASS_MUSIC_FT2PAN: Dword = 0x0000_2000;
pub const BASS_MUSIC_FT2MOD: Dword = BASS_MUSIC_FT2PAN;
pub const BASS_MUSIC_PT1MOD: Dword = 0x0000_4000;
pub const BASS_MUSIC_POSRESET: Dword = 0x0000_8000;
pub const BASS_MUSIC_NONINTER: Dword = 0x0001_0000;
pub const BASS_MUSIC_STOPBACK: Dword = BASS_STREAM_AUTOFREE << 1; // 0x00080000
pub const BASS_MUSIC_PRESCAN: Dword = BASS_STREAM_PRESCAN;
pub const BASS_MUSIC_SINCINTER: Dword = 0x0080_0000;

// ---- Channel attributes (BASS_ChannelGetAttribute/SetAttribute) ---------
pub const BASS_ATTRIB_FREQ: Dword = 1;
pub const BASS_ATTRIB_VOL: Dword = 2;
pub const BASS_ATTRIB_PAN: Dword = 3;
pub const BASS_ATTRIB_MUSIC_AMPLIFY: Dword = 0x100;
pub const BASS_ATTRIB_MUSIC_PANSEP: Dword = 0x101;

// ---- Position modes (BASS_ChannelGetPosition/SetPosition) ---------------
pub const BASS_POS_BYTE: Dword = 0;
pub const BASS_POS_MUSIC_ORDER: Dword = 1;

// ---- BASS_ChannelIsActive return values ----------------------------------
pub const BASS_ACTIVE_STOPPED: Dword = 0;
pub const BASS_ACTIVE_PLAYING: Dword = 1;
pub const BASS_ACTIVE_STALLED: Dword = 2;
pub const BASS_ACTIVE_PAUSED: Dword = 3;
pub const BASS_ACTIVE_PAUSED_DEVICE: Dword = 4;

// ---- BASS_ChannelGetData flags (combined with a length/count) -----------
pub const BASS_DATA_FLOAT: Dword = 0x4000_0000;
pub const BASS_DATA_FFT256: Dword = 0x8000_0000;
pub const BASS_DATA_FFT512: Dword = 0x8000_0001;
pub const BASS_DATA_FFT1024: Dword = 0x8000_0002;
pub const BASS_DATA_FFT2048: Dword = 0x8000_0003;
pub const BASS_DATA_FFT4096: Dword = 0x8000_0004;
pub const BASS_DATA_FFT8192: Dword = 0x8000_0005;
pub const BASS_DATA_FFT16384: Dword = 0x8000_0006;
pub const BASS_DATA_FFT32768: Dword = 0x8000_0007;
pub const BASS_DATA_AVAILABLE: Dword = 0;

// ---- Sync types (BASS_ChannelSetSync) ------------------------------------
pub const BASS_SYNC_END: Dword = 2;
/// Run the sync callback on BASS's mixer thread rather than a separate
/// notification thread; combined (OR'd) with a sync type.
pub const BASS_SYNC_MIXTIME: Dword = 0x4000_0000;

// ---- Tag types (BASS_ChannelGetTags) -------------------------------------
pub const BASS_TAG_MUSIC_NAME: Dword = 0x10000;
pub const BASS_TAG_MUSIC_MESSAGE: Dword = 0x10001;
pub const BASS_TAG_MUSIC_ORDERS: Dword = 0x10002;
/// Add the (zero-based) instrument index to this base to get
/// `BASS_TAG_MUSIC_INST` for that instrument.
pub const BASS_TAG_MUSIC_INST: Dword = 0x10300;
/// Add the (zero-based) sample index to this base to get
/// `BASS_TAG_MUSIC_SAMPLE` for that sample.
pub const BASS_TAG_MUSIC_SAMPLE: Dword = 0x10400;
/// Author/composer tag. **Unverified**: not directly confirmed against a
/// real `bass.h` in this environment (no network/DLL access) — recorded
/// here as the value documented for BASS 2.4; re-check before relying on
/// it (see PR description).
pub const BASS_TAG_MUSIC_AUTH: Dword = 0x10003;
