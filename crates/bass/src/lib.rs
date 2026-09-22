//! Safe, runtime-loaded wrapper over the [BASS 2.4](https://www.un4seen.com/)
//! audio library.
//!
//! `bass.dll` (and any add-on plugins) are **not** linked at build time and
//! are **not** shipped in this repository. They're loaded dynamically at
//! runtime with [`libloading`], from the `EMUSIC_BASS_DIR` environment
//! variable if set, otherwise a `bass/` folder next to the running
//! executable. This means:
//!
//! - The workspace builds and unit-tests fine on a machine with no BASS
//!   installation at all.
//! - [`Bass::init`] returns [`BassError::DllNotFound`] at runtime if the
//!   DLL can't be found; callers (the `app`/`player` crates) decide how to
//!   surface that (e.g. "install BASS to `bass/`").
//! - Integration tests that need a real device/DLL skip themselves when
//!   it's absent — see `tests/`.
//!
//! # Layout
//! - [`Bass`] — library lifetime (`BASS_Init`/`BASS_Free`), device
//!   enumeration, plugin loading.
//! - [`Stream`] / [`Music`] — playable channel handles, both implementing
//!   the common [`Channel`] trait (play/pause/stop, seek, attributes,
//!   FFT/sample data, ...).
//! - [`config::Config`] — global `BASS_SetConfig`/`BASS_GetConfig` knobs.
//! - [`flags`] — safe flag/enum types for stream/music creation flags,
//!   channel attributes and playback state.
//! - [`error::BassError`] — every BASS error code, plus this wrapper's own
//!   loading errors.
//! - [`ffi`] — the only place `unsafe` lives: raw types, constants and the
//!   dynamic loader. Nothing outside this module touches a raw pointer or
//!   calling convention.
//!
//! # Example
//! ```no_run
//! use bass::{Bass, Channel, StreamFlags};
//!
//! # fn main() -> Result<(), bass::BassError> {
//! let bass = Bass::init(-1, 44100)?; // -1 = default device
//! let stream = bass.open_stream("song.flac", StreamFlags::empty())?;
//! stream.play(true)?;
//! while stream.is_active() {
//!     std::thread::sleep(std::time::Duration::from_millis(100));
//! }
//! # Ok(())
//! # }
//! ```

pub mod channel;
pub mod config;
pub mod device;
pub mod error;
pub mod ffi;
pub mod flags;
mod music;
mod stream;
pub mod sync;
pub mod tags;
mod util;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use channel::{Channel, ChannelInfo};
pub use config::Config;
pub use device::DeviceInfo;
pub use error::BassError;
pub use flags::{Attribute, FftSize, MusicFlags, PlaybackState, PositionMode, StreamFlags};
pub use music::Music;
pub use stream::Stream;
pub use sync::ChannelSync;
pub use tags::MusicTags;

use ffi::{BassLib, consts as c, loader};

/// The result of attempting to load one plugin DLL via
/// [`Bass::load_plugins`].
#[derive(Debug)]
pub struct PluginLoadResult {
    /// The plugin file that was attempted.
    pub path: PathBuf,
    /// `Ok(())` if the plugin loaded successfully.
    pub result: Result<(), BassError>,
}

/// Owns the loaded BASS library and an initialized output device.
///
/// Create channels ([`Stream`], [`Music`]) from this; drop it only after
/// every channel created from it has been dropped (BASS itself will free
/// any channels still open when the device is freed, but doing so through
/// this wrapper's `Drop` ordering isn't guaranteed).
pub struct Bass {
    lib: Arc<BassLib>,
}

impl Bass {
    /// Loads `bass.dll` and initializes an output device.
    ///
    /// `device` is a zero-based device index, or `-1` for the system's
    /// default device. `freq` is the output sample rate in Hz (e.g.
    /// `44100`); pass `0` to use the device's current rate.
    pub fn init(device: i32, freq: u32) -> Result<Self, BassError> {
        let lib = Arc::new(BassLib::open()?);
        // SAFETY: `win` (window handle) and `clsid` are optional per the
        // BASS docs and null is an accepted value for both on Windows.
        let ok =
            unsafe { (lib.raw.bass_init)(device, freq, 0, std::ptr::null_mut(), std::ptr::null()) }
                != 0;
        lib.check(ok)?;
        Ok(Self { lib })
    }

    /// The loaded BASS library's version, as `(major << 24) | (minor << 16)
    /// | (rev << 8) | build`, per `BASS_GetVersion`.
    pub fn version(&self) -> u32 {
        // SAFETY: `BASS_GetVersion` takes no arguments and is always safe
        // to call once the library is loaded.
        unsafe { (self.lib.raw.bass_get_version)() }
    }

    /// Enumerates available output devices.
    pub fn devices(&self) -> Result<Vec<DeviceInfo>, BassError> {
        device::enumerate(&self.lib)
    }

    /// Global configuration knobs (buffer length, update period, resampler
    /// quality, ...).
    pub fn config(&self) -> Config {
        Config::new(Arc::clone(&self.lib))
    }

    /// Opens `path` as a new playable audio stream.
    pub fn open_stream(
        &self,
        path: impl AsRef<Path>,
        flags: StreamFlags,
    ) -> Result<Stream, BassError> {
        Stream::from_file(Arc::clone(&self.lib), path, flags)
    }

    /// Loads `path` as a new tracker module. `freq` is the mixing sample
    /// rate; `0` uses BASS's default.
    pub fn open_music(
        &self,
        path: impl AsRef<Path>,
        flags: MusicFlags,
        freq: u32,
    ) -> Result<Music, BassError> {
        Music::from_file(Arc::clone(&self.lib), path, flags, freq)
    }

    /// Loads every `bass*.dll` plugin (decoder add-on) found in `dir`,
    /// skipping `bass.dll` itself. Returns one result per file attempted,
    /// so callers can report which plugins loaded and which didn't.
    pub fn load_plugins(&self, dir: impl AsRef<Path>) -> Vec<PluginLoadResult> {
        loader::list_plugin_files(dir.as_ref())
            .into_iter()
            .map(|path| {
                let result = self.load_plugin(&path);
                PluginLoadResult { path, result }
            })
            .collect()
    }

    fn load_plugin(&self, path: &Path) -> Result<(), BassError> {
        let wide = util::path_to_utf16(path)?;
        // SAFETY: `wide` is a live, NUL-terminated UTF-16 buffer for the
        // duration of this call.
        let handle =
            unsafe { (self.lib.raw.bass_plugin_load)(wide.as_ptr().cast(), c::BASS_UNICODE) };
        if handle == 0 {
            Err(self.lib.last_error())
        } else {
            Ok(())
        }
    }
}

impl Drop for Bass {
    fn drop(&mut self) {
        // SAFETY: `BASS_Free` is safe to call any time after a successful
        // `BASS_Init`; it also frees any channels/plugins we didn't free
        // individually.
        unsafe { (self.lib.raw.bass_free)() };
    }
}
