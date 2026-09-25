#![forbid(unsafe_code)]

//! HVSC Songlengths database support (#192).
//!
//! HVSC ships a `Songlengths.md5` database: an INI-style file whose key is the
//! MD5 of a SID file's *full contents* and whose value is one `m:ss[.mmm]`
//! length per subtune. See HVSC's `DOCUMENTS/Songlengths.faq`, section "THE
//! NEW FORMAT". This module parses that format, hashes a loaded tune the same
//! way, and resolves per-subtune lengths.
//!
//! Only the new format is supported: it is the one HVSC has shipped since
//! HVSC#71 and the one present in a current installation. The legacy
//! `Songlengths.txt` (old MD5 calculation, path-keyed lookups) is left for a
//! follow-up.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use md5::{Digest, Md5};

use crate::error::SongLengthsError;

/// Per-subtune lengths, in subtune order (index 0 is subtune 1).
type Subtunes = Vec<Duration>;

/// A parsed Songlengths database, indexed by tune MD5.
#[derive(Debug, Default, Clone)]
pub struct SongLengths {
    by_md5: HashMap<[u8; 16], Subtunes>,
}

impl SongLengths {
    /// Parses new-format `Songlengths.md5` text.
    ///
    /// Blank lines, `;` comments and `[section]` headers are ignored. A
    /// malformed entry (bad key, missing `=`, unparseable duration) is skipped
    /// rather than failing the whole database, so one bad line never costs the
    /// user every tune's length.
    pub fn parse(text: &str) -> Self {
        let mut by_md5 = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Some(md5) = parse_md5(key.trim()) else {
                continue;
            };
            let Some(lengths) = parse_lengths(value) else {
                continue;
            };
            by_md5.insert(md5, lengths);
        }
        Self { by_md5 }
    }

    /// Reads and parses a database file.
    pub fn load(path: &Path) -> Result<Self, SongLengthsError> {
        let text = std::fs::read_to_string(path).map_err(|source| SongLengthsError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self::parse(&text))
    }

    /// The database key for a SID file: the MD5 of its full contents,
    /// including the header (HVSC new format).
    pub fn md5(data: &[u8]) -> [u8; 16] {
        Md5::digest(data).into()
    }

    /// Every per-subtune length for `data`, or `None` when the tune isn't in
    /// the database.
    pub fn subtunes(&self, data: &[u8]) -> Option<&[Duration]> {
        self.by_md5.get(&Self::md5(data)).map(Vec::as_slice)
    }

    /// The length of 1-based `subtune` in `data`, or `None` when the tune or
    /// that subtune isn't in the database.
    pub fn subtune(&self, data: &[u8], subtune: u16) -> Option<Duration> {
        let index = usize::from(subtune.checked_sub(1)?);
        self.subtunes(data)?.get(index).copied()
    }

    /// How many tunes the database holds.
    pub fn len(&self) -> usize {
        self.by_md5.len()
    }

    /// Whether the database holds no tunes at all.
    pub fn is_empty(&self) -> bool {
        self.by_md5.is_empty()
    }
}

/// Resolves a user-configured path to a Songlengths file.
///
/// Accepts the file itself, or an HVSC root folder, where the database is
/// auto-detected at `DOCUMENTS/Songlengths.md5` (the layout HVSC ships).
pub fn resolve_database_path(configured: &Path) -> Option<PathBuf> {
    if configured.is_file() {
        return Some(configured.to_path_buf());
    }
    if configured.is_dir() {
        for relative in ["DOCUMENTS/Songlengths.md5", "Songlengths.md5"] {
            let candidate = configured.join(relative);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Parses a 32-hex-character MD5 key.
fn parse_md5(text: &str) -> Option<[u8; 16]> {
    if text.len() != 32 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Parses the space-separated length list of one entry.
fn parse_lengths(value: &str) -> Option<Subtunes> {
    let lengths: Vec<Duration> = value
        .split_whitespace()
        .map(parse_duration)
        .collect::<Option<_>>()?;
    (!lengths.is_empty()).then_some(lengths)
}

/// Parses one `m:ss[.SSS]` length. Minutes never carry a leading zero and are
/// unbounded; seconds are `0..60`; milliseconds are the optional fractional
/// part (1..=3 digits, but any float representation is accepted).
fn parse_duration(token: &str) -> Option<Duration> {
    let (minutes, seconds) = token.split_once(':')?;
    if minutes.is_empty() || (minutes.len() > 1 && minutes.starts_with('0')) {
        return None;
    }
    let minutes: u64 = minutes.parse().ok()?;
    if seconds.is_empty() {
        return None;
    }
    let seconds: f64 = seconds.parse().ok()?;
    if !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Some(Duration::from_secs_f64(minutes as f64 * 60.0 + seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a one-entry database for `data` with the given length tokens.
    fn database_for(data: &[u8], lengths: &str) -> String {
        let key: String = SongLengths::md5(data)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("[Database]\n; a comment\n\n{key}={lengths}\n")
    }

    #[test]
    fn parses_an_entry_and_looks_it_up_by_full_content_md5() {
        let data = b"a tune's bytes";
        let db = SongLengths::parse(&database_for(data, "3:57 1:02 0:06"));
        assert_eq!(db.len(), 1);
        assert_eq!(
            db.subtunes(data),
            Some([3 * 60 + 57, 62, 6].map(Duration::from_secs).as_slice())
        );
    }

    #[test]
    fn supports_millisecond_lengths() {
        let data = b"millis";
        let db = SongLengths::parse(&database_for(data, "2:03.108 9:14 1:02.5"));
        assert_eq!(
            db.subtunes(data),
            Some(vec![
                Duration::from_secs_f64(123.108),
                Duration::from_secs(554),
                Duration::from_secs_f64(62.5),
            ])
            .as_deref()
        );
    }

    #[test]
    fn subtune_index_is_one_based_and_out_of_range_is_none() {
        let data = b"subtunes";
        let db = SongLengths::parse(&database_for(data, "0:40 0:50"));
        assert_eq!(db.subtune(data, 1), Some(Duration::from_secs(40)));
        assert_eq!(db.subtune(data, 2), Some(Duration::from_secs(50)));
        assert_eq!(db.subtune(data, 0), None);
        assert_eq!(db.subtune(data, 3), None);
    }

    #[test]
    fn an_unknown_tune_is_not_found() {
        let db = SongLengths::parse(&database_for(b"other", "1:00"));
        assert_eq!(db.subtunes(b"unknown"), None);
        assert_eq!(db.subtune(b"unknown", 1), None);
    }

    #[test]
    fn malformed_lines_are_skipped_without_losing_valid_ones() {
        let data = b"valid";
        let valid = database_for(data, "1:02");
        let text = format!(
            "{valid}\
             ; comment\n\
             not-a-hash=1:00\n\
             0123456789abcdef0123456789abcdef\n\
             0123456789abcdef0123456789abcdef=oops\n\
             0123456789abcdef0123456789abcdef=1:60\n\
             [Database]\n"
        );
        let db = SongLengths::parse(&text);
        assert_eq!(db.len(), 1, "only the valid entry survives");
        assert_eq!(db.subtune(data, 1), Some(Duration::from_secs(62)));
    }

    #[test]
    fn rejects_leading_zero_minutes_and_out_of_range_seconds() {
        assert!(parse_duration("01:00").is_none());
        assert!(parse_duration("1:60").is_none());
        assert!(parse_duration("1:").is_none());
        assert!(parse_duration(":30").is_none());
        assert!(parse_duration("junk").is_none());
        assert_eq!(parse_duration("0:00"), Some(Duration::ZERO));
    }

    #[test]
    fn md5_matches_the_known_empty_digest() {
        let hex: String = SongLengths::md5(b"")
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(hex, "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn resolve_database_path_accepts_a_file_and_a_root_folder() {
        // Build the fixtures in the temp dir rather than pointing at a real
        // file in the source tree: an absolute path baked in at compile time
        // (like `CARGO_MANIFEST_DIR`) does not exist in another environment,
        // e.g. inside Windows Sandbox.
        let temp = std::env::temp_dir();
        let file_root = temp.join(format!("emusic-sid-sl-file-{}", std::process::id()));
        std::fs::create_dir_all(&file_root).expect("create temp dir");
        let file = file_root.join("Songlengths.md5");
        std::fs::write(&file, "[Database]\n").expect("write database");
        assert_eq!(resolve_database_path(&file), Some(file.clone()));
        let _ = std::fs::remove_dir_all(&file_root);

        let root = temp.join(format!("emusic-sid-sl-root-{}", std::process::id()));
        let documents = root.join("DOCUMENTS");
        std::fs::create_dir_all(&documents).expect("create temp HVSC root");
        let database = documents.join("Songlengths.md5");
        std::fs::write(&database, "[Database]\n").expect("write database");
        assert_eq!(resolve_database_path(&root), Some(database));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_database_path_returns_none_for_a_missing_path() {
        assert_eq!(
            resolve_database_path(Path::new("Z:/definitely/absent/Songlengths.md5")),
            None
        );
    }

    /// Verifies the hashing rule against a real HVSC installation. Set
    /// `EMUSIC_SID_SONGLENGTHS` to a `Songlengths.md5` and `EMUSIC_SID_TUNE`
    /// to any `.sid` inside that HVSC tree; the test then asserts the file is
    /// found by its full-content MD5 and has at least one subtune length.
    /// Skips (rather than fails) when either variable is unset, like the BASS
    /// integration tests.
    #[test]
    fn real_hvsc_database_matches_a_real_tune_when_configured() {
        let (Ok(database), Ok(tune)) = (
            std::env::var("EMUSIC_SID_SONGLENGTHS"),
            std::env::var("EMUSIC_SID_TUNE"),
        ) else {
            return;
        };
        let db = SongLengths::load(Path::new(&database)).expect("load the real database");
        assert!(!db.is_empty());
        let data = std::fs::read(tune).expect("read the tune");
        let lengths = db.subtunes(&data).expect("tune should be in the database");
        assert!(!lengths.is_empty());
    }
}
