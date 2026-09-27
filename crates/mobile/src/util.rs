#![forbid(unsafe_code)]

//! Small filesystem and formatting helpers shared by the connector.

use std::path::Path;

use emusic_render::{
    MODULE_EXTENSIONS, ModuleRenderer, Renderer, SID_EXTENSIONS, SidPlayerRenderer,
};

/// Replaces `destination` with `source`, staying atomic on Unix.
///
/// On Unix `rename` replaces atomically; on Windows it fails when the
/// destination exists, so fall back to removing it first.
pub(crate) fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) => {
            if destination.exists() {
                std::fs::remove_file(destination)?;
                std::fs::rename(source, destination)
            } else {
                Err(error)
            }
        }
    }
}

/// Maps a server format label to a safe file extension, falling back to `bin`.
pub(crate) fn safe_extension(format: &str) -> String {
    if !format.is_empty()
        && format.len() <= 16
        && format.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        format.to_ascii_lowercase()
    } else {
        "bin".to_string()
    }
}

/// The local renderer for a format, if one exists.
pub(crate) fn renderer_for(format: &str, sample_rate: u32) -> Option<Box<dyn Renderer + Send>> {
    if MODULE_EXTENSIONS
        .iter()
        .any(|candidate| format.eq_ignore_ascii_case(candidate))
    {
        return Some(Box::new(ModuleRenderer::new(sample_rate)));
    }
    if SID_EXTENSIONS
        .iter()
        .any(|candidate| format.eq_ignore_ascii_case(candidate))
    {
        return Some(Box::new(SidPlayerRenderer::new(sample_rate)));
    }
    None
}

/// Percent-encodes a value for use as a single URL path segment.
pub(crate) fn encode_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_extension_sanitizes() {
        assert_eq!(safe_extension("MOD"), "mod");
        assert_eq!(safe_extension("a/b"), "bin");
        assert_eq!(safe_extension(""), "bin");
    }

    #[test]
    fn renderer_for_matches_known_formats() {
        assert!(renderer_for("mod", 44_100).is_some());
        assert!(renderer_for("sid", 44_100).is_some());
        assert!(renderer_for("flac", 44_100).is_none());
    }

    #[test]
    fn replace_file_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.tmp");
        let destination = dir.path().join("a");
        std::fs::write(&destination, b"old").unwrap();
        std::fs::write(&source, b"new").unwrap();
        replace_file(&source, &destination).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"new");
    }
}
