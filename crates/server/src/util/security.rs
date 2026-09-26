//! Path traversal protection, safe canonicalization, rate limiting, and trusted proxy parsing.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Path contains forbidden null bytes or invalid characters")]
    InvalidPathCharacters,
    #[error("Path traversal detected or path outside library root")]
    PathTraversal,
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolves `relative_path` relative to one of the authorized `base_roots`.
///
/// Ensures strict path canonicalization:
/// - Rejects null bytes and invalid characters.
/// - Canonicalizes the target path and verifies `target.starts_with(base_root)`.
/// - Prevents symlink breakouts outside designated library roots.
pub fn canonicalize_and_validate_path(
    base_roots: &[PathBuf],
    relative_path: impl AsRef<Path>,
) -> Result<PathBuf, SecurityError> {
    let rel = relative_path.as_ref();
    let rel_str = rel.to_string_lossy();

    if rel_str.contains('\0') {
        return Err(SecurityError::InvalidPathCharacters);
    }

    // Strip leading path separators to ensure relative join
    let clean_rel = rel_str.trim_start_matches(['/', '\\']);
    let clean_path = Path::new(clean_rel);

    for root in base_roots {
        let Ok(canonical_root) = dunce::canonicalize(root) else {
            continue;
        };

        let candidate = canonical_root.join(clean_path);
        if let Ok(canonical_candidate) = dunce::canonicalize(&candidate) {
            if canonical_candidate.starts_with(&canonical_root) {
                if canonical_candidate.is_file() {
                    return Ok(canonical_candidate);
                }
            } else {
                // Resolved path escaped the base root
                return Err(SecurityError::PathTraversal);
            }
        }
    }

    Err(SecurityError::NotFound(rel_str.to_string()))
}

/// In-memory rate limiter tracking IP attempt windows.
#[derive(Debug, Clone, Default)]
pub struct RateLimiter {
    attempts: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if the request from `ip` is within `max_attempts` for the given `window`.
    /// Records the attempt if allowed.
    pub fn check_and_record(&self, ip: IpAddr, max_attempts: u32, window: Duration) -> bool {
        let mut map = self.attempts.lock().unwrap_or_else(|p| p.into_inner());
        let now = Instant::now();

        let attempts = map.entry(ip).or_default();
        attempts.retain(|time| now.duration_since(*time) < window);

        if attempts.len() >= max_attempts as usize {
            false
        } else {
            attempts.push(now);
            true
        }
    }
}

/// Extracts real client IP address given peer socket address and headers if peer is in trusted proxies.
pub fn extract_client_ip(
    peer_ip: IpAddr,
    forwarded_for: Option<&str>,
    trusted_proxies: &[String],
) -> IpAddr {
    let peer_str = peer_ip.to_string();
    let is_trusted = trusted_proxies
        .iter()
        .any(|proxy| proxy == &peer_str || proxy == "*");

    if is_trusted
        && let Some(header_val) = forwarded_for
        && let Some(first_ip) = header_val.split(',').next()
        && let Ok(parsed_ip) = first_ip.trim().parse::<IpAddr>()
    {
        return parsed_ip;
    }

    peer_ip
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_canonicalize_and_validate_path_valid() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let file_path = root.join("song.mp3");
        std::fs::write(&file_path, b"audio").unwrap();

        let result = canonicalize_and_validate_path(&[root], "song.mp3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_canonicalize_and_validate_path_traversal() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("music");
        std::fs::create_dir_all(&root).unwrap();
        let outside_file = dir.path().join("secret.txt");
        std::fs::write(&outside_file, b"secret").unwrap();

        let result = canonicalize_and_validate_path(&[root], "../secret.txt");
        assert!(matches!(
            result,
            Err(SecurityError::NotFound(_)) | Err(SecurityError::PathTraversal)
        ));
    }

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new();
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        let window = Duration::from_secs(60);

        assert!(limiter.check_and_record(ip, 2, window));
        assert!(limiter.check_and_record(ip, 2, window));
        assert!(!limiter.check_and_record(ip, 2, window));
    }

    #[test]
    fn test_extract_client_ip() {
        let proxy_ip: IpAddr = "172.16.0.1".parse().unwrap();
        let trusted = vec!["172.16.0.1".to_string()];

        let client_ip = extract_client_ip(proxy_ip, Some("203.0.113.195, 172.16.0.1"), &trusted);
        assert_eq!(client_ip, "203.0.113.195".parse::<IpAddr>().unwrap());

        let untrusted_ip: IpAddr = "192.168.1.50".parse().unwrap();
        let client_ip2 = extract_client_ip(untrusted_ip, Some("203.0.113.195"), &trusted);
        assert_eq!(client_ip2, untrusted_ip);
    }
}
