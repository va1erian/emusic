//! Per-channel soundfont control via BASSMIDI's own exports
//! (`bassmidi.dll`), on top of the plugin [`crate::Bass::load_plugins`]
//! already registers for MIDI decoding.
//!
//! [`crate::config::Config::set_midi_default_font`]
//! (`BASS_CONFIG_MIDI_DEFFONT`) only supplies the font a *newly opened* MIDI
//! stream starts with — BASSMIDI captures the config's value once, when the
//! stream is created. [`Midi::set_channel_font`] is the mechanism BASSMIDI
//! documents for changing an **already open, possibly playing** channel's
//! soundfont, taking effect immediately.

use std::path::Path;
use std::sync::Arc;

use crate::error::BassError;
use crate::ffi::MidiLib;
use crate::ffi::types::{BassMidiFont, Dword, HSoundFont};
use crate::util::path_to_utf16;

/// A soundfont (`.sf2`/`.sf3`/`.sfz`) loaded via `BASS_MIDI_FontInit`.
///
/// Freed automatically (`BASS_MIDI_FontFree`) when dropped; drop it only
/// after every channel it was applied to has moved on to a different font
/// (or been closed), matching BASSMIDI's own requirement.
pub struct SoundFont {
    midi: Arc<MidiLib>,
    handle: HSoundFont,
}

impl SoundFont {
    fn load(midi: Arc<MidiLib>, path: &Path) -> Result<Self, BassError> {
        let wide = path_to_utf16(path)?;
        // SAFETY: `wide` is a live, NUL-terminated UTF-16 buffer for the
        // duration of this call, matching the `BASS_UNICODE` flag; BASS
        // reads the font from disk rather than keeping the pointer.
        let handle = unsafe {
            (midi.raw.bass_midi_font_init)(wide.as_ptr().cast(), crate::ffi::consts::BASS_UNICODE)
        };
        if handle == 0 {
            return Err(midi.last_error());
        }
        Ok(Self { midi, handle })
    }
}

impl Drop for SoundFont {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was returned by a successful
        // `BASS_MIDI_FontInit` call and hasn't been freed yet (`SoundFont`
        // owns it exclusively).
        unsafe { (self.midi.raw.bass_midi_font_free)(self.handle) };
    }
}

/// Handle to `bassmidi.dll`'s own exports, for per-channel soundfont
/// control. Obtained via [`crate::Bass::load_midi`].
#[derive(Clone)]
pub struct Midi {
    lib: Arc<MidiLib>,
}

impl Midi {
    pub(crate) fn new(lib: Arc<MidiLib>) -> Self {
        Self { lib }
    }

    /// Loads `path` as a soundfont, ready to apply to one or more channels.
    pub fn load_soundfont(&self, path: &Path) -> Result<SoundFont, BassError> {
        SoundFont::load(Arc::clone(&self.lib), path)
    }

    /// Applies `font` to `channel` (a raw handle from
    /// [`crate::Channel::handle`]), replacing whatever font(s) it had.
    /// Takes effect immediately, including on a channel that's already open
    /// and playing. Pass `channel = 0` to set the default new MIDI streams
    /// start with instead of one specific channel.
    pub fn set_channel_font(&self, channel: Dword, font: &SoundFont) -> Result<(), BassError> {
        let entry = BassMidiFont {
            font: font.handle,
            preset: -1,
            bank: 0,
        };
        self.set_channel_fonts(channel, &[entry])
    }

    fn set_channel_fonts(&self, channel: Dword, fonts: &[BassMidiFont]) -> Result<(), BassError> {
        let count = fonts.len() as Dword;
        // SAFETY: `fonts` is a valid, readable array of `count`
        // `BASS_MIDI_FONT` entries for the duration of this call; `channel`
        // is either `0` (BASSMIDI's documented "set the default" handle) or
        // a real channel handle the caller owns.
        let result =
            unsafe { (self.lib.raw.bass_midi_stream_set_fonts)(channel, fonts.as_ptr(), count) };
        // `count` fonts requested but none set back is BASSMIDI's failure
        // signal; `count == 0` (clearing) legitimately returns 0 too.
        if count > 0 && result == 0 {
            return Err(self.lib.last_error());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // `Midi`/`SoundFont` always talk to a loaded `bassmidi.dll`, so their
    // behaviour is only exercised by the DLL-gated integration tests
    // (`tests/`). Nothing here is pure enough to unit test without it.
}
