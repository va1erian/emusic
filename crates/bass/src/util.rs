//! Small helpers shared across the safe API modules.

use std::path::Path;

use crate::error::BassError;

/// Converts a filesystem path to a NUL-terminated UTF-16 buffer suitable
/// for passing to a BASS function alongside the `BASS_UNICODE` flag.
///
/// Returns [`BassError::InvalidPath`] if the path contains an embedded NUL
/// character, which can't be represented in a NUL-terminated string.
pub(crate) fn path_to_utf16(path: &Path) -> Result<Vec<u16>, BassError> {
    let mut wide: Vec<u16> = path.to_string_lossy().encode_utf16().collect::<Vec<u16>>();
    if wide.contains(&0) {
        return Err(BassError::InvalidPath(format!(
            "path contains embedded NUL: {}",
            path.display()
        )));
    }
    wide.push(0);
    Ok(wide)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn encodes_ascii_path_with_nul_terminator() {
        let wide = path_to_utf16(&PathBuf::from("C:\\music\\song.flac")).unwrap();
        assert_eq!(*wide.last().unwrap(), 0);
        let text = String::from_utf16(&wide[..wide.len() - 1]).unwrap();
        assert_eq!(text, "C:\\music\\song.flac");
    }

    #[test]
    fn encodes_non_ascii_path() {
        let wide = path_to_utf16(&PathBuf::from("C:\\música\\ファイル.mp3")).unwrap();
        let text = String::from_utf16(&wide[..wide.len() - 1]).unwrap();
        assert_eq!(text, "C:\\música\\ファイル.mp3");
    }

    #[test]
    fn rejects_embedded_nul() {
        let path = PathBuf::from(String::from("bad\0path"));
        assert!(matches!(
            path_to_utf16(&path),
            Err(BassError::InvalidPath(_))
        ));
    }
}
