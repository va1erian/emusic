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

/// Reads a single UTF-16 tag string, or `None` if BASS has nothing for it.
fn read_tag(lib: &BassLib, channel: Dword, tag: Dword) -> Option<String> {
    // SAFETY: OR-ing in `BASS_UNICODE` asks BASS to return a `wchar_t*`
    // instead of `char*`; the returned pointer (if non-null) is owned by
    // BASS and remains valid until the channel is freed or the tag is
    // re-queried, which is longer than we need it for here.
    let ptr = unsafe { (lib.raw.bass_channel_get_tags)(channel, tag | c::BASS_UNICODE) };
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` is a non-null, NUL-terminated UTF-16 string per the
    // `BASS_UNICODE` request above.
    let text = unsafe { wide_cstr_to_string(ptr as *const u16) };
    if text.is_empty() { None } else { Some(text) }
}

/// Reads an indexed series of tags (`base`, `base + 1`, ...) until BASS
/// returns null, e.g. `BASS_TAG_MUSIC_INST` / `BASS_TAG_MUSIC_SAMPLE`.
fn read_tag_list(lib: &BassLib, channel: Dword, base: Dword) -> Vec<String> {
    let mut items = Vec::new();
    let mut index: Dword = 0;
    loop {
        // SAFETY: same reasoning as `read_tag`; `BASS_UNICODE` requests a
        // wide string, and a null return ends the list per BASS's
        // documented behaviour for indexed tags.
        let ptr =
            unsafe { (lib.raw.bass_channel_get_tags)(channel, (base + index) | c::BASS_UNICODE) };
        if ptr.is_null() {
            break;
        }
        // SAFETY: see `read_tag`.
        items.push(unsafe { wide_cstr_to_string(ptr as *const u16) });
        index += 1;
    }
    items
}

/// # Safety
/// `ptr` must be non-null and point to a NUL-terminated UTF-16 string that
/// remains valid for the duration of this call.
unsafe fn wide_cstr_to_string(ptr: *const u16) -> String {
    let mut len = 0usize;
    // SAFETY: caller guarantees `ptr` points to a NUL-terminated UTF-16
    // buffer; we only read up to and including the terminator.
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    // SAFETY: `ptr..ptr+len` was just proven readable above.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf16_lossy(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_cstr_reads_up_to_terminator() {
        let mut buf: Vec<u16> = "hello".encode_utf16().collect();
        buf.push(0);
        buf.push('!' as u16); // must not be read
        // SAFETY: `buf` is a valid NUL-terminated UTF-16 buffer for the
        // duration of this call.
        let text = unsafe { wide_cstr_to_string(buf.as_ptr()) };
        assert_eq!(text, "hello");
    }
}
