//! `BASS_SetConfig`/`BASS_GetConfig` helpers.
//!
//! These affect BASS globally and can be called before or after
//! [`crate::Bass::init`].

use std::path::Path;
use std::sync::Arc;

use crate::error::BassError;
use crate::ffi::{BassLib, consts as c};
use crate::util::path_to_utf16;

/// Global BASS configuration knobs exposed by this crate.
///
/// Cloning is cheap: it shares the same loaded library handle used by
/// [`crate::Bass`].
#[derive(Clone)]
pub struct Config {
    lib: Arc<BassLib>,
}

impl Config {
    pub(crate) fn new(lib: Arc<BassLib>) -> Self {
        Self { lib }
    }

    fn set(&self, option: u32, value: u32) -> Result<(), BassError> {
        // SAFETY: `BASS_SetConfig` accepts any `option`/`value` pair; an
        // unrecognised option is simply ignored by BASS (it returns
        // `FALSE`, which we surface as an error).
        let ok = unsafe { (self.lib.raw.bass_set_config)(option, value) } != 0;
        self.lib.check(ok)
    }

    fn get(&self, option: u32) -> u32 {
        // SAFETY: `BASS_GetConfig` accepts any `option`; unrecognised
        // options return `0xFFFFFFFF` (i.e. `u32::MAX`) or `0`, we don't
        // interpret it here.
        unsafe { (self.lib.raw.bass_get_config)(option) }
    }

    /// Sets the playback buffer length, in milliseconds.
    pub fn set_buffer_length_ms(&self, ms: u32) -> Result<(), BassError> {
        self.set(c::BASS_CONFIG_BUFFER, ms)
    }

    /// Current playback buffer length, in milliseconds.
    pub fn buffer_length_ms(&self) -> u32 {
        self.get(c::BASS_CONFIG_BUFFER)
    }

    /// Sets how often BASS updates playback buffers, in milliseconds.
    pub fn set_update_period_ms(&self, ms: u32) -> Result<(), BassError> {
        self.set(c::BASS_CONFIG_UPDATEPERIOD, ms)
    }

    /// Current update period, in milliseconds.
    pub fn update_period_ms(&self) -> u32 {
        self.get(c::BASS_CONFIG_UPDATEPERIOD)
    }

    /// Sets the sample rate conversion quality (`0`-`4`, higher is
    /// better/slower) used by streams/musics that don't specify their own.
    pub fn set_resampling_quality(&self, quality: u32) -> Result<(), BassError> {
        self.set(c::BASS_CONFIG_SRC, quality)
    }

    /// Sets the default soundfont (`.sf2`/`.sf3`/`.dls`) BASSMIDI plays MIDI
    /// files with. Needs `bassmidi.dll` loaded as a plugin; without it BASS
    /// ignores the option and this returns an error.
    pub fn set_midi_default_font(&self, path: &Path) -> Result<(), BassError> {
        let wide = path_to_utf16(path)?;
        // SAFETY: `wide` is a live NUL-terminated UTF-16 buffer for the
        // duration of the call, matching the `BASS_UNICODE` flag; BASS copies
        // the string rather than keeping the pointer.
        let ok = unsafe {
            (self.lib.raw.bass_set_config_ptr)(
                c::BASS_CONFIG_MIDI_DEFFONT | c::BASS_UNICODE,
                wide.as_ptr().cast(),
            )
        } != 0;
        self.lib.check(ok)
    }

    /// Current default sample rate conversion quality.
    pub fn resampling_quality(&self) -> u32 {
        self.get(c::BASS_CONFIG_SRC)
    }
}

#[cfg(test)]
mod tests {
    // `Config` always talks to a loaded `BassLib`, so its behaviour is only
    // exercised by the DLL-gated integration tests (`tests/`). Nothing here
    // is pure enough to unit test without a loaded library.
}
