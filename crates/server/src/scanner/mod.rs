use std::path::Path;
use std::sync::Arc;
use walkdir::WalkDir;
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::db::{DbStore, TrackRecord};

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "ogg", "opus", "aac", "m4a",
    "sid", "psid", "rsid",
    "mod", "s3m", "xm", "it", "mo3",
    "mid", "midi"
];

pub struct LibraryScanner {
    db: Arc<DbStore>,
}

impl LibraryScanner {
    pub fn new(db: Arc<DbStore>) -> Self {
        Self { db }
    }

    pub fn scan(&self, root_paths: &[impl AsRef<Path>]) {
        let version = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for root in root_paths {
            let root_path = root.as_ref();
            if !root_path.exists() {
                warn!("Scan path does not exist: {}", root_path.display());
                continue;
            }

            info!("Scanning library at {}", root_path.display());

            for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension().and_then(|e| e.to_str())
                {
                    let ext_lower = ext.to_lowercase();
                    if SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str())
                        && let Ok(record) = self.create_track_record(root_path, path, &ext_lower, version)
                    {
                        let _ = self.db.upsert_track(&record);
                    }
                }
            }
        }
    }

    fn create_track_record(
        &self,
        root: &Path,
        file: &Path,
        format: &str,
        version: u64,
    ) -> Result<TrackRecord, Box<dyn std::error::Error>> {
        let relative = file.strip_prefix(root)?;
        let relative_str = relative.to_string_lossy().replace('\\', "/");

        let metadata = std::fs::metadata(file)?;
        let file_size = metadata.len();
        let mtime = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let mut hasher = Sha256::new();
        hasher.update(relative_str.as_bytes());
        hasher.update(file_size.to_le_bytes());
        hasher.update(mtime.to_le_bytes());
        let hash_hex = hex::encode(hasher.finalize());
        let id = hash_hex[..16].to_string();

        let title = file.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string());

        Ok(TrackRecord {
            id,
            relative_path: relative_str,
            format: format.to_string(),
            title,
            artist: None,
            album: None,
            duration_secs: None,
            subtunes: 1,
            file_size,
            mtime,
            hash: hash_hex,
            version,
        })
    }
}
