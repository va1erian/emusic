//! Cover-art discovery and extraction.
//!
//! Artwork is either embedded in a track's tags or stored as a conventional
//! file next to it (`cover.jpg`, `folder.png`, ...). Extraction is bounded to
//! avoid serving unexpectedly large images.

use std::path::{Path, PathBuf};

/// Maximum size of an artwork file served.
pub const MAX_ART_BYTES: u64 = 12 * 1024 * 1024;

/// File stems treated as cover art.
const ART_STEMS: &[&str] = &[
    "cover", "folder", "front", "album", "albumart", "thumb", "artwork",
];

/// Extensions treated as cover art.
const ART_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp"];

/// Extracted artwork bytes and content type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artwork {
    /// Image bytes.
    pub bytes: Vec<u8>,
    /// MIME type.
    pub mime: String,
}

/// Finds a conventional cover-art file in `dir`, case-insensitively.
pub fn find_art_file(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let Some((stem, ext)) = name.rsplit_once('.') else {
            continue;
        };
        if ART_STEMS.contains(&stem) && ART_EXTS.contains(&ext) {
            return Some(entry.path());
        }
    }
    None
}

/// Extracts artwork for a track: embedded first, then a sidecar file.
pub fn extract(path: &Path) -> Option<Artwork> {
    if let Some(art) = extract_embedded(path) {
        return Some(art);
    }
    let dir = path.parent()?;
    let art_path = find_art_file(dir)?;
    let mime = mime_for(path_to_ext(&art_path).as_deref()).to_string();
    let bytes = read_capped(&art_path, MAX_ART_BYTES)?;
    Some(Artwork { bytes, mime })
}

fn extract_embedded(path: &Path) -> Option<Artwork> {
    use lofty::file::TaggedFileExt;
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = tag.pictures().first()?;
    let data = picture.data();
    if data.is_empty() || data.len() as u64 > MAX_ART_BYTES {
        return None;
    }
    let mime = picture
        .mime_type()
        .map(|mime| mime.as_str().to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    Some(Artwork {
        bytes: data.to_vec(),
        mime,
    })
}

fn read_capped(path: &Path, max: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > max {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn path_to_ext(path: &Path) -> Option<String> {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
}

/// MIME type for a cover-art file extension.
pub fn mime_for(ext: Option<&str>) -> &'static str {
    match ext {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "emusic-srv-artx-{name}-{}-{}",
            std::process::id(),
            crate::util::unix_now()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_sidecar_art_case_insensitively() {
        let dir = temp_dir("find");
        std::fs::write(dir.join("Cover.PNG"), b"img").unwrap();
        let found = find_art_file(&dir).expect("found");
        assert_eq!(extract(&dir.join("song.flac")).unwrap().mime, "image/png");
        assert!(found.is_file());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_art_returns_none() {
        let dir = temp_dir("none");
        std::fs::write(dir.join("song.flac"), b"not audio").unwrap();
        assert!(extract(&dir.join("song.flac")).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mime_mapping() {
        assert_eq!(mime_for(Some("jpg")), "image/jpeg");
        assert_eq!(mime_for(Some("jpeg")), "image/jpeg");
        assert_eq!(mime_for(Some("png")), "image/png");
        assert_eq!(mime_for(None), "application/octet-stream");
    }
}
