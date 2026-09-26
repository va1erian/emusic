//! Turning a discovered file into a `NewTrack` row.
//!
//! Metadata extraction is best-effort: a file whose tags cannot be parsed is
//! still indexed with its filesystem metadata and counted as "skipped" so the
//! operator can see it in the scan report.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::db::models::NewTrack;
use crate::util::{md5_hex, sha256_hex};

use super::formats;
use super::songlengths::SongLengths;
use super::walk::FoundFile;
use super::{module, sid, tags};

/// `stream` kind label.
const KIND_STREAM: &str = "stream";

/// Maximum number of bytes read for SID hashing.
const SID_HASH_LIMIT: u64 = 8 * 1024 * 1024;

/// Header bytes needed for module parsing.
const MODULE_HEADER_BYTES: u64 = 8 * 1024;

/// Cache of "does this directory contain cover art".
#[derive(Debug, Default)]
pub struct ArtCache {
    dirs: HashMap<PathBuf, bool>,
}

impl ArtCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    fn has_art(&mut self, dir: &Path) -> bool {
        if let Some(found) = self.dirs.get(dir) {
            return *found;
        }
        let found = super::art::find_art_file(dir).is_some();
        self.dirs.insert(dir.to_path_buf(), found);
        found
    }
}

/// Builds the stored row for one file. Returns the row and whether metadata
/// parsing failed (the row is still produced).
pub fn index_file(
    file: &FoundFile,
    root_index: i64,
    songlengths: &SongLengths,
    art: &mut ArtCache,
    now: i64,
) -> (NewTrack, bool) {
    let id = track_id(root_index, &file.relative_path);
    let hash = fingerprint(file.size, file.mtime);
    let mut track = NewTrack {
        id,
        root_index,
        relative_path: file.relative_path.clone(),
        format: file.format.label.to_string(),
        kind: if file.format.kind == "module" {
            "module".to_string()
        } else {
            KIND_STREAM.to_string()
        },
        title: None,
        artist: None,
        album_artist: None,
        album: None,
        album_id: None,
        genre: None,
        year: None,
        track_no: None,
        disc_no: None,
        duration_secs: None,
        subtunes: 1,
        channels: None,
        file_size: file.size,
        mtime: file.mtime,
        hash,
        has_art: false,
        added_at: now,
    };

    let mut skipped = false;
    let ext = file
        .path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    if formats::is_sid(&ext) {
        apply_sid(&mut track, &file.path, songlengths);
    } else if file.format.kind == "module" {
        apply_module(&mut track, &file.path);
    } else if formats::is_midi(&ext) {
        // MIDI carries no tags lofty reads; the client derives its title.
    } else {
        match tags::read_stream(&file.path) {
            Ok(fields) => apply_stream(&mut track, fields),
            Err(_) => skipped = true,
        }
    }

    if track.has_art {
        // Already known embedded; nothing more to do.
    } else if let Some(dir) = file.path.parent() {
        track.has_art = art.has_art(dir);
    }
    track.album_id = album_identifier(
        track.album_artist.as_deref().or(track.artist.as_deref()),
        track.album.as_deref(),
    );
    (track, skipped)
}

/// Derives the opaque album identifier from album artist and album name.
///
/// Case and surrounding whitespace are ignored so minor tag differences do
/// not split one album in two. Returns `None` without an album name.
pub fn album_identifier(artist: Option<&str>, album: Option<&str>) -> Option<String> {
    let album = album?.trim();
    if album.is_empty() {
        return None;
    }
    let artist = artist.unwrap_or("").trim().to_lowercase();
    let material = format!("{artist}\0{}", album.to_lowercase());
    Some(sha256_hex(b"emusic-server/album/v1:", material.as_bytes()))
}

fn apply_stream(track: &mut NewTrack, fields: tags::TagFields) {
    track.title = fields.title;
    track.artist = fields.artist;
    track.album_artist = fields.album_artist;
    track.album = fields.album;
    track.genre = fields.genre;
    track.year = fields.year;
    track.track_no = fields.track_no;
    track.disc_no = fields.disc_no;
    track.duration_secs = fields.duration_secs;
    track.channels = fields.channels;
    track.has_art = fields.has_embedded_art;
}

fn apply_sid(track: &mut NewTrack, path: &Path, songlengths: &SongLengths) {
    let Some(data) = read_all_capped(path, SID_HASH_LIMIT) else {
        return;
    };
    if let Some(header) = sid::parse(&data) {
        track.subtunes = header.songs;
        track.title = header.name.clone();
        track.artist = header.author.clone();
        track.channels = None;
    }
    let md5 = md5_hex(&data);
    if let Some(durations) = songlengths.durations(&md5) {
        if let Some(first) = durations.first() {
            track.duration_secs = Some(*first);
        }
        if durations.len() > track.subtunes as usize {
            track.subtunes = durations.len() as u32;
        }
    }
}

fn apply_module(track: &mut NewTrack, path: &Path) {
    let Some(data) = read_prefix(path, MODULE_HEADER_BYTES) else {
        return;
    };
    let info = module::parse(&data);
    track.title = info.title;
    track.channels = info.channels;
}

/// The track identifier: SHA-256 over the root index and the relative path.
pub fn track_id(root_index: i64, relative_path: &str) -> String {
    let material = format!("{root_index}\0{relative_path}");
    sha256_hex(b"emusic-server/track/v1:", material.as_bytes())
}

/// Cheap change fingerprint over size and mtime.
fn fingerprint(size: u64, mtime: i64) -> String {
    sha256_hex(
        b"emusic-server/fingerprint/v1:",
        format!("{size}:{mtime}").as_bytes(),
    )
}

fn read_all_capped(path: &Path, max: u64) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut data = Vec::new();
    file.take(max).read_to_end(&mut data).ok()?;
    Some(data)
}

fn read_prefix(path: &Path, max: u64) -> Option<Vec<u8>> {
    read_all_capped(path, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_id_is_stable_and_root_specific() {
        let a = track_id(0, "album/song.flac");
        assert_eq!(a, track_id(0, "album/song.flac"));
        assert_ne!(a, track_id(1, "album/song.flac"));
        assert_ne!(a, track_id(0, "album/other.flac"));
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn album_identifier_is_case_and_artist_sensitive() {
        let a = album_identifier(Some("Artist"), Some("Album"));
        assert_eq!(a, album_identifier(Some("artist"), Some(" album ")));
        assert_ne!(a, album_identifier(Some("Other"), Some("Album")));
        assert_ne!(a, album_identifier(Some("Artist"), Some("Other")));
        assert_eq!(album_identifier(Some("Artist"), None), None);
        assert_eq!(album_identifier(Some("Artist"), Some("   ")), None);
    }

    #[test]
    fn external_cover_detection_is_case_insensitive() {
        let dir = std::env::temp_dir().join(format!(
            "emusic-srv-art-{}-{}",
            std::process::id(),
            crate::util::unix_now()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Folder.JPG"), b"x").unwrap();
        let mut cache = ArtCache::new();
        assert!(cache.has_art(&dir));
        assert!(cache.has_art(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_cover_means_no_art() {
        let dir = std::env::temp_dir().join(format!(
            "emusic-srv-noart-{}-{}",
            std::process::id(),
            crate::util::unix_now()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        assert!(!ArtCache::new().has_art(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }
}
