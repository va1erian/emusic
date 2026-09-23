//! Path classification and normalisation for the scanner.
//!
//! Library roots may be mapped drive letters (`Z:\music`) or UNC shares
//! (`\\nas\music`), and the same file can be spelled several ways. To keep
//! scan results stable, paths are never canonicalised with
//! [`std::fs::canonicalize`] (which yields `\\?\UNC\...` verbatim paths);
//! instead they are simplified with [`dunce`] — a pure string operation —
//! and compared through a case-insensitive, separator-normalised key.

use std::path::Path;

use emusic_core::TrackKind;

/// File extensions read as streamed audio (tags and properties via lofty).
pub(crate) const STREAM_EXTENSIONS: &[&str] = &[
    "aac", "afc", "aif", "aifc", "aiff", "ape", "flac", "m4a", "m4b", "m4p", "mp+", "mp2", "mp3",
    "mpc", "mpp", "ogg", "opus", "spx", "wav", "wv",
];

/// MIDI file extensions, read via BASS (`bassmidi` plugin). They are stored
/// as [`TrackKind::Stream`] tracks but carry no tags for lofty to read.
pub(crate) const MIDI_EXTENSIONS: &[&str] = &["mid", "midi"];

/// Tracker module extensions, read via BASS (`BASS_MusicLoad`).
pub(crate) const MODULE_EXTENSIONS: &[&str] = &["it", "mo3", "mod", "mtm", "s3m", "umx", "xm"];

/// Classifies `path` as a scannable audio file by its extension.
pub(crate) fn classify(path: &Path) -> Option<TrackKind> {
    let ext = extension(path)?;
    if STREAM_EXTENSIONS.contains(&ext.as_str()) || MIDI_EXTENSIONS.contains(&ext.as_str()) {
        Some(TrackKind::Stream)
    } else if MODULE_EXTENSIONS.contains(&ext.as_str()) {
        Some(TrackKind::Module)
    } else {
        None
    }
}

/// Whether `path` names a MIDI file.
pub(crate) fn is_midi(path: &Path) -> bool {
    extension(path).is_some_and(|ext| MIDI_EXTENSIONS.contains(&ext.as_str()))
}

/// The lowercased file extension of `path`, without the leading dot.
pub(crate) fn extension(path: &Path) -> Option<String> {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
}

/// A comparison key for a path: simplified, lowercased, forward slashes.
///
/// `Z:\Music\A.flac`, `z:/music/a.flac` and `\\?\Z:\Music\A.flac` all
/// normalise to `z:/music/a.flac`, so a file opened from Explorer via a
/// mapped drive matches the library row stored for the UNC spelling.
pub(crate) fn normalize_key(path: &Path) -> String {
    let text = path.to_string_lossy();
    let without_verbatim = text
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| text.strip_prefix(r"\\?\").map(String::from))
        .unwrap_or_else(|| text.into_owned());
    let lowered = without_verbatim.to_lowercase();
    lowered.replace('\\', "/")
}

/// Whether `key` is `root_key` itself or lives underneath it.
///
/// Both arguments must already be normalised with [`normalize_key`].
pub(crate) fn key_is_under(key: &str, root_key: &str) -> bool {
    key == root_key
        || key
            .strip_prefix(root_key)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn classify_recognises_stream_and_module_extensions() {
        assert_eq!(classify(Path::new("a.FLAC")), Some(TrackKind::Stream));
        assert_eq!(classify(Path::new("b.Mp3")), Some(TrackKind::Stream));
        assert_eq!(classify(Path::new("d.MID")), Some(TrackKind::Stream));
        assert_eq!(classify(Path::new("e.midi")), Some(TrackKind::Stream));
        assert!(is_midi(Path::new("e.Midi")));
        assert!(!is_midi(Path::new("e.mp3")));
        assert_eq!(classify(Path::new("c.xm")), Some(TrackKind::Module));
        assert_eq!(classify(Path::new("c.IT")), Some(TrackKind::Module));
    }

    #[test]
    fn classify_rejects_unsupported_and_extensionless_paths() {
        assert_eq!(classify(Path::new("notes.txt")), None);
        assert_eq!(classify(Path::new("readme")), None);
        assert_eq!(classify(Path::new("music/")), None);
    }

    #[test]
    fn normalize_key_unifies_case_separators_and_verbatim_prefixes() {
        let mapped = normalize_key(&PathBuf::from(r"Z:\Music\A.flac"));
        let forward = normalize_key(&PathBuf::from("z:/music/a.flac"));
        let verbatim = normalize_key(&PathBuf::from(r"\\?\Z:\Music\A.flac"));
        assert_eq!(mapped, "z:/music/a.flac");
        assert_eq!(mapped, forward);
        assert_eq!(mapped, verbatim);
    }

    #[test]
    fn normalize_key_unifies_mapped_and_verbatim_unc_shares() {
        let unc = normalize_key(&PathBuf::from(r"\\nas\music\A.flac"));
        let verbatim = normalize_key(&PathBuf::from(r"\\?\UNC\nas\music\A.flac"));
        assert_eq!(unc, "//nas/music/a.flac");
        assert_eq!(unc, verbatim);
    }

    #[test]
    fn key_is_under_matches_root_and_descendants_only() {
        let root = "z:/music";
        assert!(key_is_under("z:/music", root));
        assert!(key_is_under("z:/music/sub/a.flac", root));
        assert!(!key_is_under("z:/musician/a.flac", root));
        assert!(!key_is_under("z:/other/a.flac", root));
    }
}
