//! HVSC `Songlengths.md5` parsing.
//!
//! The file maps the MD5 of each SID tune to one or more subtune lengths:
//!
//! ```text
//! ; comments start with a semicolon
//! 003a54e4...=2:51 4:12.500
//! ```
//!
//! Parsing is tolerant: malformed lines and durations are skipped rather than
//! aborting the load.

use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;

/// Song lengths keyed by lowercase MD5 of the SID file contents.
#[derive(Debug, Default, Clone)]
pub struct SongLengths {
    entries: HashMap<String, Vec<f64>>,
}

impl SongLengths {
    /// An empty table.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Loads lengths from a `Songlengths.md5` file.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(Self::parse(&text))
    }

    /// Parses lengths from the file's contents.
    pub fn parse(text: &str) -> Self {
        let mut entries: HashMap<String, Vec<f64>> = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
                continue;
            }
            let Some((key, values)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            if key.len() != 32 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                continue;
            }
            let durations: Vec<f64> = values
                .split_whitespace()
                .filter_map(parse_duration)
                .collect();
            if !durations.is_empty() {
                entries.entry(key).or_default().extend(durations);
            }
        }
        Self { entries }
    }

    /// Durations for a tune, in subtune order.
    pub fn durations(&self, md5: &str) -> Option<&[f64]> {
        self.entries.get(md5).map(Vec::as_slice)
    }

    /// Number of indexed tunes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Iterates over every tune's MD5 and durations.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[f64])> {
        self.entries
            .iter()
            .map(|(key, values)| (key.as_str(), values.as_slice()))
    }

    /// Whether the table has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Parses `m:ss`, `mm:ss`, `m:ss.mmm` or plain seconds into seconds.
fn parse_duration(token: &str) -> Option<f64> {
    let (minutes, rest) = match token.split_once(':') {
        Some((minutes, rest)) => (minutes.parse::<u64>().ok()?, rest),
        None => (0, token),
    };
    let seconds = rest.parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some(minutes as f64 * 60.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_and_comments() {
        let text = "; comment\n[Database]\n\
            003a54e4e4b3a26b64a53b5b2a5e5b7f=2:51 4:12.5\n\
            aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=0:30\n";
        let lengths = SongLengths::parse(text);
        assert_eq!(lengths.len(), 2);
        let first = lengths
            .durations("003a54e4e4b3a26b64a53b5b2a5e5b7f")
            .unwrap();
        assert_eq!(first.len(), 2);
        assert!((first[0] - 171.0).abs() < f64::EPSILON);
        assert!((first[1] - 252.5).abs() < f64::EPSILON);
        assert_eq!(lengths.durations("unknown"), None);
    }

    #[test]
    fn skips_malformed_lines_and_values() {
        let text = "not a hash=1:00\n\
            aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa=\n\
            bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb=abc\n\
            003a54e4e4b3a26b64a53b5b2a5e5b7f=1:00 garbage\n";
        let lengths = SongLengths::parse(text);
        assert!(
            lengths
                .durations("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
                .is_none()
        );
        assert_eq!(
            lengths
                .durations("003a54e4e4b3a26b64a53b5b2a5e5b7f")
                .unwrap(),
            &[60.0]
        );
    }

    #[test]
    fn parse_duration_forms() {
        assert_eq!(parse_duration("1:00"), Some(60.0));
        assert_eq!(parse_duration("10:00.5"), Some(600.5));
        assert_eq!(parse_duration("45"), Some(45.0));
        assert_eq!(parse_duration("garbage"), None);
        assert_eq!(parse_duration("1:-1"), None);
    }
}
