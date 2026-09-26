//! Local caching of remote streams and raw binary demoscene files (.sid, .mod, .xm, .it, .s3m, .mid).

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Invalid path or URL")]
    InvalidTarget,
}

pub struct StreamCacheManager {
    cache_dir: PathBuf,
}

impl StreamCacheManager {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Result<Self, CacheError> {
        let dir = cache_dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { cache_dir: dir })
    }

    pub fn default_location() -> Result<Self, CacheError> {
        let base = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
        let cache_dir = base.join("emusic").join("cache").join("tracks");
        Self::new(cache_dir)
    }

    /// Resolves `url_or_path`: if local file, returns path directly.
    /// If HTTP/HTTPS URL, checks local cache or downloads into cache without panicking.
    pub fn resolve_file(
        &self,
        url_or_path: &str,
        auth_token: Option<&str>,
    ) -> Result<PathBuf, CacheError> {
        let path = Path::new(url_or_path);
        if path.is_file() {
            return Ok(path.to_path_buf());
        }

        if !url_or_path.starts_with("http://") && !url_or_path.starts_with("https://") {
            return Err(CacheError::InvalidTarget);
        }

        let mut hasher = Sha256::new();
        hasher.update(url_or_path.as_bytes());
        let hash_hex = hex::encode(hasher.finalize());

        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("tmp");

        let cached_file = self.cache_dir.join(format!("{hash_hex}.{ext}"));
        if cached_file.is_file()
            && let Ok(meta) = fs::metadata(&cached_file)
            && meta.len() > 0
        {
            return Ok(cached_file);
        }

        // Fetch from remote server safely
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| CacheError::Http(e.to_string()))?;

        let mut req = client.get(url_or_path);
        if let Some(token) = auth_token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }

        let res = req.send().map_err(|e| CacheError::Http(e.to_string()))?;
        if !res.status().is_success() {
            return Err(CacheError::Http(format!("HTTP {}", res.status())));
        }

        let bytes = res.bytes().map_err(|e| CacheError::Http(e.to_string()))?;

        let temp_file = self.cache_dir.join(format!("{hash_hex}.tmp"));
        {
            let mut f = File::create(&temp_file)?;
            f.write_all(&bytes)?;
            f.flush()?;
        }

        fs::rename(&temp_file, &cached_file)?;
        Ok(cached_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_file_resolution() {
        let temp_dir = std::env::temp_dir();
        let manager = StreamCacheManager::new(&temp_dir).unwrap();
        let local_file = temp_dir.join("test.txt");
        fs::write(&local_file, b"test").unwrap();

        let resolved = manager
            .resolve_file(local_file.to_str().unwrap(), None)
            .unwrap();
        assert_eq!(resolved, local_file);
    }
}
