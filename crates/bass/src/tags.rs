//! `BASS_ChannelGetTags`: MOD music metadata (name, message, instrument and
//! sample names).

use crate::ffi::BassLib;
use crate::ffi::consts as c;
use crate::ffi::types::Dword;

/// Metadata tags read from a MOD music channel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MusicTags {
    /// The tracker module's title, if present.
    pub name: Option<String>,
    /// The module's embedded message/comment text, if present.
    pub message: Option<String>,
    /// Instrument names, in order (index = instrument number).
    pub instruments: Vec<String>,
    /// Sample names, in order (index = sample number).
    pub samples: Vec<String>,
}

/// Reads every tag this crate knows about from a MOD music channel.
pub(crate) fn read_music_tags(lib: &BassLib, channel: Dword) -> MusicTags {
    MusicTags {
        name: read_tag(lib, channel, c::BASS_TAG_MUSIC_NAME),
        message: read_tag(lib, channel, c::BASS_TAG_MUSIC_MESSAGE),
        instruments: read_tag_list(lib, channel, c::BASS_TAG_MUSIC_INST),
        samples: read_tag_list(lib, channel, c::BASS_TAG_MUSIC_SAMPLE),
    }
}

/// Reads a single tag string, or `None` if BASS has nothing for it.
///
/// Unlike most other `BASS_ChannelGetTags` types (ID3, RIFF INFO, ...), the
/// MOD-specific tags (`BASS_TAG_MUSIC_*`) do not support `BASS_UNICODE`:
/// requesting one with that flag OR'd in returns null with
/// `BASS_ErrorGetCode() == BASS_ERROR_NOTAVAIL`, confirmed empirically
/// against a real `bass.dll` while verifying #138 (every module tag was
/// silently empty). BASS always returns these as a narrow, NUL-terminated
/// C string instead.
fn read_tag(lib: &BassLib, channel: Dword, tag: Dword) -> Option<String> {
    // SAFETY: `tag` is one of the `BASS_TAG_MUSIC_*` constants; per the
    // doc comment above, BASS returns a `char*` for these regardless of
    // `BASS_UNICODE`. The returned pointer (if non-null) is owned by BASS
    // and remains valid until the channel is freed or the tag is
    // re-queried, which is longer than we need it for here.
    let ptr = unsafe { (lib.raw.bass_channel_get_tags)(channel, tag) };
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` is a non-null, NUL-terminated narrow C string per the
    // doc comment above.
    let text = unsafe { narrow_cstr_to_string(ptr as *const std::ffi::c_char) };
    if text.is_empty() { None } else { Some(text) }
}

/// Reads an indexed series of tags (`base`, `base + 1`, ...) until BASS
/// returns null, e.g. `BASS_TAG_MUSIC_INST` / `BASS_TAG_MUSIC_SAMPLE`.
fn read_tag_list(lib: &BassLib, channel: Dword, base: Dword) -> Vec<String> {
    let mut items = Vec::new();
    let mut index: Dword = 0;
    loop {
        // SAFETY: same reasoning as `read_tag`; a null return ends the
        // list per BASS's documented behaviour for indexed tags.
        let ptr = unsafe { (lib.raw.bass_channel_get_tags)(channel, base + index) };
        if ptr.is_null() {
            break;
        }
        // SAFETY: see `read_tag`.
        items.push(unsafe { narrow_cstr_to_string(ptr as *const std::ffi::c_char) });
        index += 1;
    }
    items
}

/// # Safety
/// `ptr` must be non-null and point to a NUL-terminated narrow C string
/// that remains valid for the duration of this call.
///
/// Decoded as UTF-8 (lossily): module tags are usually plain ASCII, so this
/// is correct in the common case; a tracker that wrote extended/accented
/// characters in a DOS-era codepage (e.g. CP437) may show replacement
/// characters instead of the intended glyphs. Properly honoring the
/// original codepage is a possible follow-up, not attempted here.
unsafe fn narrow_cstr_to_string(ptr: *const std::ffi::c_char) -> String {
    // SAFETY: caller guarantees `ptr` is non-null and NUL-terminated.
    let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
    cstr.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_cstr_reads_up_to_terminator() {
        let mut buf: Vec<std::ffi::c_char> =
            "hello".bytes().map(|b| b as std::ffi::c_char).collect();
        buf.push(0);
        buf.push(b'!' as std::ffi::c_char); // must not be read
        // SAFETY: `buf` is a valid NUL-terminated narrow C string for the
        // duration of this call.
        let text = unsafe { narrow_cstr_to_string(buf.as_ptr()) };
        assert_eq!(text, "hello");
    }
}
