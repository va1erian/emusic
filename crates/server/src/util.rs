//! Small shared helpers with no dependencies on other server modules.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current time as a Unix timestamp in seconds, or `0` if the clock is
/// before the epoch (which never happens on a correctly configured host).
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Formats Unix seconds as `YYYY-MM-DD HH:MM:SS UTC` (the raw number when out
/// of range).
pub fn format_utc(unix: i64) -> String {
    match time::OffsetDateTime::from_unix_timestamp(unix) {
        Ok(at) => format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
            at.year(),
            u8::from(at.month()),
            at.day(),
            at.hour(),
            at.minute(),
            at.second()
        ),
        Err(_) => unix.to_string(),
    }
}

/// Hex-encoded SHA-256 of `bytes`, with an optional domain-separation prefix.
pub fn sha256_hex(prefix: &[u8], bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Lowercase hex MD5 of `bytes`, used to key HVSC song lengths.
pub fn md5_hex(bytes: &[u8]) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_now_is_sane() {
        assert!(unix_now() > 1_700_000_000);
    }

    #[test]
    fn format_utc_renders_utc() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc(1_700_000_000), "2023-11-14 22:13:20 UTC");
    }

    #[test]
    fn sha256_hex_is_deterministic_and_prefix_separated() {
        let a = sha256_hex(b"a:", b"payload");
        assert_eq!(a, sha256_hex(b"a:", b"payload"));
        assert_ne!(a, sha256_hex(b"b:", b"payload"));
        assert_eq!(a.len(), 64);
    }
}
