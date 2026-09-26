//! Library file crawler and metadata scanner.

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

use crate::db::Database;
use crate::db::models::ServerTrack;

pub fn scan_library(base_roots: &[impl AsRef<Path>], db: &Database) -> usize {
    let mut scanned = 0;

    for root in base_roots {
        let root_path = root.as_ref();
        let Ok(canonical_root) = dunce::canonicalize(root_path) else {
            continue;
        };

        for entry in WalkDir::new(&canonical_root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if let Some(track) = process_file(&canonical_root, path)
                && db.upsert_track(&track).is_ok()
            {
                scanned += 1;
            }
        }
    }

    scanned
}

fn process_file(root: &Path, file_path: &Path) -> Option<ServerTrack> {
    let rel_path = file_path.strip_prefix(root).ok()?;
    let rel_str = rel_path.to_string_lossy().to_string();

    let ext = file_path.extension()?.to_str()?.to_lowercase();
    if !is_supported_extension(&ext) {
        return None;
    }

    let metadata = std::fs::metadata(file_path).ok()?;
    let file_size = metadata.len();
    let mtime = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;

    // Track ID is SHA256 of canonical relative path
    let mut hasher = Sha256::new();
    hasher.update(rel_str.as_bytes());
    let id = hex::encode(hasher.finalize());

    let subtunes = if ext == "sid" || ext == "psid" || ext == "rsid" {
        read_sid_subtunes(file_path)
    } else {
        1
    };

    let (title, artist, album, duration_secs) = read_tags(file_path);

    Some(ServerTrack {
        id: id.clone(),
        relative_path: rel_str,
        format: ext,
        title,
        artist,
        album,
        duration_secs,
        subtunes,
        file_size,
        mtime,
        hash: id,
    })
}

fn is_supported_extension(ext: &str) -> bool {
    matches!(
        ext,
        "mp3"
            | "flac"
            | "wav"
            | "ogg"
            | "opus"
            | "aac"
            | "m4a"
            | "sid"
            | "psid"
            | "rsid"
            | "mod"
            | "s3m"
            | "xm"
            | "it"
            | "mo3"
            | "mid"
            | "midi"
    )
}

fn read_sid_subtunes(path: &Path) -> u32 {
    if let Ok(mut f) = File::open(path) {
        let mut header = [0u8; 16];
        if f.read_exact(&mut header).is_ok()
            && (&header[0..4] == b"PSID" || &header[0..4] == b"RSID")
        {
            let songs = u16::from_be_bytes([header[14], header[15]]);
            if songs > 0 {
                return songs as u32;
            }
        }
    }
    1
}

fn read_tags(path: &Path) -> (Option<String>, Option<String>, Option<String>, Option<f64>) {
    let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) else {
        return (None, None, None, None);
    };

    let properties = tagged_file.properties();
    let duration = Some(properties.duration().as_secs_f64());

    if let Some(tag) = tagged_file.primary_tag() {
        let title = tag.title().as_deref().map(|s| s.to_string());
        let artist = tag.artist().as_deref().map(|s| s.to_string());
        let album = tag.album().as_deref().map(|s| s.to_string());
        (title, artist, album, duration)
    } else {
        (None, None, None, duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_scanner_processes_files() {
        let dir = tempdir().unwrap();
        let music_dir = dir.path().join("music");
        std::fs::create_dir_all(&music_dir).unwrap();

        let track_path = music_dir.join("test.mp3");
        std::fs::write(&track_path, b"fake mp3 data").unwrap();

        let db = Database::in_memory().unwrap();
        let scanned = scan_library(&[music_dir], &db);
        assert_eq!(scanned, 1);

        let tracks = db.list_tracks(0).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].format, "mp3");
    }
}
