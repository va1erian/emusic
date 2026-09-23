#![forbid(unsafe_code)]

//! Parser for HVSC's INI-style `Songlengths.md5` database.
//!
//! Format (see `Songlengths.faq`):
//! ```text
//! [Database]
//! ; /DEMOS/0-9/10_Orbyte.sid
//! 5f08a730b280e54fd1e75a7046b93fdc=1:17
//! ```

use std::collections::HashMap;
use std::time::Duration;

/// Parses the database into an MD5 -> per-subtune lengths map.
pub(super) fn parse(contents: &str) -> HashMap<[u8; 16], Vec<Duration>> {
    let mut lengths = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(digest) = parse_md5(key.trim()) else {
            continue;
        };
        let per_subtune: Vec<Duration> = value
            .split_whitespace()
            .filter_map(parse_length)
            .collect();
        if !per_subtune.is_empty() {
            lengths.insert(digest, per_subtune);
        }
    }
    lengths
}

/// Parses a 32-character hex MD5.
fn parse_md5(text: &str) -> Option<[u8; 16]> {
    if text.len() != 32 {
        return None;
    }
    let mut digest = [0u8; 16];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(digest)
}

/// Parses a `mm:ss[.SSS]` song length.
fn parse_length(text: &str) -> Option<Duration> {
    let (minutes, rest) = text.split_once(':')?;
    let minutes: u64 = minutes.parse().ok()?;
    let (seconds, millis) = match rest.split_once('.') {
        Some((seconds, fraction)) => (seconds, millis_of(fraction)?),
        None => (rest, 0),
    };
    let seconds: u64 = seconds.parse().ok()?;
    Some(Duration::from_millis(
        minutes * 60_000 + seconds * 1_000 + millis,
    ))
}

/// Converts a 1..=3 digit fractional-seconds field to milliseconds.
fn millis_of(fraction: &str) -> Option<u64> {
    if fraction.is_empty() || fraction.len() > 3 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value: u64 = fraction.parse().ok()?;
    Some(match fraction.len() {
        1 => value * 100,
        2 => value * 10,
        _ => value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_database_with_comments_and_sections() {
        let text = "[Database]\n; /a/Tune.sid\n\
            c4c5ff8cfefdf683c50e66775cfac1ee=3:57 1:02 0:06 0:02\n\
            ; /b/Other.sid\n\
            00000000000000000000000000000000=1:02.5\n";
        let map = parse(text);
        assert_eq!(map.len(), 2);

        let first = &map[&parse_md5("c4c5ff8cfefdf683c50e66775cfac1ee").unwrap()];
        assert_eq!(first.len(), 4);
        assert_eq!(first[0], Duration::from_secs(237));
        assert_eq!(first[3], Duration::from_secs(2));
    }

    #[test]
    fn parses_optional_milliseconds() {
        assert_eq!(parse_length("1:02"), Some(Duration::from_secs(62)));
        assert_eq!(parse_length("1:02.5"), Some(Duration::from_millis(62_500)));
        assert_eq!(parse_length("1:02.50"), Some(Duration::from_millis(62_500)));
        assert_eq!(parse_length("1:02.500"), Some(Duration::from_millis(62_500)));
        assert_eq!(parse_length("0:00.923"), Some(Duration::from_millis(923)));
        assert_eq!(parse_length("nonsense"), None);
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(parse("garbage\n=1:00\n1234=1:00\n").is_empty());
    }
}
