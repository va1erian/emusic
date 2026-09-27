//! Small helpers shared across the safe API modules.

use std::ffi::c_void;
use std::path::Path;

use crate::error::BassError;
#[cfg(windows)]
use crate::ffi::consts::BASS_UNICODE;
use crate::ffi::types::Dword;

/// A filesystem path encoded for a BASS call.
///
/// BASS wants a NUL-terminated UTF-16 buffer plus the `BASS_UNICODE` flag on
/// Windows, and an (unchanged) NUL-terminated UTF-8 string with no flag on
/// macOS: its narrow file APIs are UTF-8 there, so non-ASCII paths work
/// without the wide-character variant.
pub(crate) struct BassPath {
    /// UTF-16 code units on Windows, UTF-8 bytes elsewhere.
    #[cfg(windows)]
    buffer: Vec<u16>,
    #[cfg(not(windows))]
    buffer: Vec<u8>,
}

impl BassPath {
    /// Encodes `path`, rejecting an embedded NUL (which a NUL-terminated
    /// string can't represent).
    pub(crate) fn new(path: &Path) -> Result<Self, BassError> {
        #[cfg(windows)]
        {
            let mut buffer: Vec<u16> = path.to_string_lossy().encode_utf16().collect();
            if buffer.contains(&0) {
                return Err(invalid_path(path));
            }
            buffer.push(0);
            Ok(Self { buffer })
        }
        #[cfg(not(windows))]
        {
            let mut buffer = path.to_string_lossy().into_owned().into_bytes();
            if buffer.contains(&0) {
                return Err(invalid_path(path));
            }
            buffer.push(0);
            Ok(Self { buffer })
        }
    }

    /// The pointer to pass to the BASS call: a `wchar_t*` on Windows, a
    /// `char*` elsewhere. Valid for as long as `self`.
    pub(crate) fn as_ptr(&self) -> *const c_void {
        self.buffer.as_ptr().cast()
    }

    /// The filename-encoding flag for the call this path is passed to:
    /// `BASS_UNICODE` on Windows, `0` elsewhere.
    pub(crate) fn flag(&self) -> Dword {
        #[cfg(windows)]
        {
            BASS_UNICODE
        }
        #[cfg(not(windows))]
        {
            0
        }
    }
}

/// The error for a path that can't be encoded as a NUL-terminated string.
fn invalid_path(path: &Path) -> BassError {
    BassError::InvalidPath(format!("path contains embedded NUL: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The path text back out of an encoded buffer, checking the terminator.
    fn text(encoded: &BassPath) -> String {
        #[cfg(windows)]
        {
            assert_eq!(*encoded.buffer.last().unwrap(), 0);
            String::from_utf16(&encoded.buffer[..encoded.buffer.len() - 1]).unwrap()
        }
        #[cfg(not(windows))]
        {
            assert_eq!(*encoded.buffer.last().unwrap(), 0);
            String::from_utf8(encoded.buffer[..encoded.buffer.len() - 1].to_vec()).unwrap()
        }
    }

    #[test]
    fn encodes_ascii_path_with_nul_terminator() {
        let encoded = BassPath::new(&PathBuf::from(r"C:\music\song.flac")).unwrap();
        assert_eq!(text(&encoded), r"C:\music\song.flac");
    }

    #[test]
    fn encodes_non_ascii_path() {
        let encoded = BassPath::new(&PathBuf::from("C:\\música\\ファイル.mp3")).unwrap();
        assert_eq!(text(&encoded), "C:\\música\\ファイル.mp3");
    }

    #[test]
    fn rejects_embedded_nul() {
        let path = PathBuf::from(String::from("bad\0path"));
        assert!(matches!(
            BassPath::new(&path),
            Err(BassError::InvalidPath(_))
        ));
    }

    #[test]
    fn unicode_flag_matches_the_encoding() {
        let encoded = BassPath::new(Path::new("song.flac")).unwrap();
        #[cfg(windows)]
        assert_eq!(encoded.flag(), BASS_UNICODE);
        #[cfg(not(windows))]
        assert_eq!(encoded.flag(), 0);
    }
}
