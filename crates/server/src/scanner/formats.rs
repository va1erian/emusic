//! Audio format classification and MIME types.
//!
//! The server decides per file whether it is ordinary streamed audio, a
//! tracker module, a SID tune or MIDI. SID/module/MIDI files are delivered raw
//! so the client keeps its native rendering, channel inspection and
//! visualisation features (see `docs/server-design.md`).

/// Classification of a file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatInfo {
    /// Lowercase label stored in the database and sent to clients.
    pub label: &'static str,
    /// `stream` or `module`.
    pub kind: &'static str,
    /// Content type used when streaming.
    pub mime: &'static str,
    /// Whether the raw file must be delivered for client-side rendering.
    pub specialized: bool,
}

const STREAM: &str = "stream";
const MODULE: &str = "module";

/// Extensions read as ordinary tagged audio.
const STREAM_FORMATS: &[(&str, &str)] = &[
    ("aac", "audio/aac"),
    ("aif", "audio/aiff"),
    ("aifc", "audio/aiff"),
    ("aiff", "audio/aiff"),
    ("ape", "audio/x-ape"),
    ("flac", "audio/flac"),
    ("m4a", "audio/mp4"),
    ("m4b", "audio/mp4"),
    ("mp2", "audio/mpeg"),
    ("mp3", "audio/mpeg"),
    ("mpc", "audio/x-musepack"),
    ("mpp", "audio/x-musepack"),
    ("ogg", "audio/ogg"),
    ("opus", "audio/opus"),
    ("spx", "audio/ogg"),
    ("wav", "audio/wav"),
    ("wv", "audio/x-wavpack"),
];

/// Tracker module extensions rendered by the client via BASS.
const MODULE_FORMATS: &[&str] = &["it", "mo3", "mod", "mtm", "s3m", "umx", "xm"];

/// SID tune extensions rendered by the client via cRSID.
const SID_FORMATS: &[&str] = &["sid", "psid", "rsid"];

/// MIDI extensions rendered by the client via BASSMIDI.
const MIDI_FORMATS: &[&str] = &["mid", "midi"];

/// Classifies a lowercased extension.
pub fn classify(ext: &str) -> Option<FormatInfo> {
    if let Some((label, mime)) = STREAM_FORMATS.iter().find(|(label, _)| *label == ext) {
        return Some(FormatInfo {
            label,
            kind: STREAM,
            mime,
            specialized: false,
        });
    }
    if let Some(label) = MODULE_FORMATS.iter().find(|label| **label == ext) {
        return Some(FormatInfo {
            label,
            kind: MODULE,
            mime: "audio/x-mod",
            specialized: true,
        });
    }
    if let Some(label) = SID_FORMATS.iter().find(|label| **label == ext) {
        return Some(FormatInfo {
            label,
            kind: STREAM,
            mime: "audio/x-sid",
            specialized: true,
        });
    }
    if let Some(label) = MIDI_FORMATS.iter().find(|label| **label == ext) {
        return Some(FormatInfo {
            label,
            kind: STREAM,
            mime: "audio/midi",
            specialized: true,
        });
    }
    None
}

/// Whether an extension names a SID tune.
pub fn is_sid(ext: &str) -> bool {
    SID_FORMATS.contains(&ext)
}

/// Whether an extension names a MIDI file.
pub fn is_midi(ext: &str) -> bool {
    MIDI_FORMATS.contains(&ext)
}

/// Lowercased extension of a path, without the dot.
pub fn extension_of(path: &std::path::Path) -> Option<String> {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn classifies_standard_formats() {
        let info = classify("flac").unwrap();
        assert_eq!(info.label, "flac");
        assert_eq!(info.kind, STREAM);
        assert!(!info.specialized);
        assert_eq!(info.mime, "audio/flac");
    }

    #[test]
    fn classifies_specialized_formats() {
        assert!(classify("sid").unwrap().specialized);
        assert!(classify("psid").unwrap().specialized);
        assert!(classify("mid").unwrap().specialized);
        assert!(classify("mod").unwrap().specialized);
        assert_eq!(classify("xm").unwrap().kind, MODULE);
        assert_eq!(classify("mo3").unwrap().mime, "audio/x-mod");
        assert_eq!(classify("midi").unwrap().mime, "audio/midi");
    }

    #[test]
    fn rejects_unknown_extensions() {
        assert!(classify("txt").is_none());
        assert!(classify("").is_none());
        assert!(classify("flac ").is_none());
    }

    #[test]
    fn extension_of_lowercases() {
        assert_eq!(extension_of(Path::new("A.FLAC")).as_deref(), Some("flac"));
        assert_eq!(extension_of(Path::new("noext")), None);
    }
}
