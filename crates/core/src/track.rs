use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::TrackId;

/// Where a track's artwork should be loaded from.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArtSource {
    /// No known artwork.
    #[default]
    None,
    /// Artwork embedded in the track's own tag data.
    Embedded,
    /// Artwork found alongside the track (e.g. `folder.jpg`).
    ExternalFile(PathBuf),
}

/// The kind of audio content a track represents.
///
/// This distinguishes normal streamed audio (mp3, flac, ...) from tracker
/// module files (mod, xm, it, s3m, ...), which BASS decodes differently and
/// which have their own settings panel (see the plan's "tracker module
/// controls").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    /// A regular streamed audio file.
    Stream,
    /// A tracker module file.
    Module,
}

/// A single track in the library, as scanned from disk and tagged.
///
/// Fields mirror what's persisted in the `tracks` table (see
/// `emusic-library`); optional metadata fields are `None` when absent from
/// the file's tags rather than defaulted to empty strings, so UI code can
/// tell "unknown" apart from "empty".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Store-assigned identifier; [`TrackId::UNASSIGNED`] before it is
    /// first persisted.
    pub id: TrackId,
    /// Full path to the file on disk.
    pub path: PathBuf,
    /// Parent directory of `path`, kept denormalized for fast folder
    /// grouping and incremental scans.
    pub dir: PathBuf,
    /// File name including extension.
    pub filename: String,
    /// Lowercased file extension, without the leading dot.
    pub ext: String,
    /// File size in bytes.
    pub size: u64,
    /// File modification time, as a Unix timestamp (seconds, UTC).
    pub mtime: i64,
    /// Whether this is a streamed file or a tracker module.
    pub kind: TrackKind,
    /// Duration in milliseconds.
    pub duration_ms: u32,
    /// Bitrate in kbps, when known (typically absent for modules).
    pub bitrate: Option<u32>,
    /// Sample rate in Hz, when known.
    pub sample_rate: Option<u32>,
    /// Channel count, when known.
    pub channels: Option<u8>,
    /// Tagged title.
    pub title: Option<String>,
    /// Tagged track artist.
    pub artist: Option<String>,
    /// Tagged album artist, used for grouping compilations.
    pub album_artist: Option<String>,
    /// Tagged album name.
    pub album: Option<String>,
    /// Tagged genre.
    pub genre: Option<String>,
    /// Tagged release year.
    pub year: Option<i32>,
    /// Tagged track number within the album.
    pub track_no: Option<u32>,
    /// Tagged disc number within the release.
    pub disc_no: Option<u32>,
    /// Tagged composer.
    pub composer: Option<String>,
    /// Free-form tagged comment.
    pub comment: Option<String>,
    /// Where to load artwork for this track from.
    pub art_source: ArtSource,
    /// When this track was first added to the library, as a Unix timestamp
    /// (seconds, UTC).
    pub added_at: i64,
    /// Whether the user has starred (favorited) this track (#131).
    pub starred: bool,
}

impl Track {
    /// Title to show in the UI: the tagged title, falling back to the file
    /// name when no title tag is present.
    pub fn display_title(&self) -> &str {
        match &self.title {
            Some(title) if !title.trim().is_empty() => title,
            _ => &self.filename,
        }
    }

    /// Album artist to group by: the tagged album artist, falling back to
    /// the track artist when no album artist tag is present.
    pub fn effective_album_artist(&self) -> Option<&str> {
        match &self.album_artist {
            Some(album_artist) if !album_artist.trim().is_empty() => Some(album_artist),
            _ => self.artist.as_deref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_track() -> Track {
        Track {
            id: TrackId::UNASSIGNED,
            path: PathBuf::from(r"C:\music\Artist\Album\01 Song.flac"),
            dir: PathBuf::from(r"C:\music\Artist\Album"),
            filename: "01 Song.flac".to_string(),
            ext: "flac".to_string(),
            size: 12_345,
            mtime: 1_700_000_000,
            kind: TrackKind::Stream,
            duration_ms: 210_000,
            bitrate: Some(1_000),
            sample_rate: Some(44_100),
            channels: Some(2),
            title: None,
            artist: Some("Some Artist".to_string()),
            album_artist: None,
            album: Some("Some Album".to_string()),
            genre: None,
            year: Some(2020),
            track_no: Some(1),
            disc_no: None,
            composer: None,
            comment: None,
            art_source: ArtSource::None,
            added_at: 1_700_000_000,
            starred: false,
        }
    }

    #[test]
    fn display_title_falls_back_to_filename() {
        let track = sample_track();
        assert_eq!(track.display_title(), "01 Song.flac");
    }

    #[test]
    fn display_title_prefers_tagged_title() {
        let mut track = sample_track();
        track.title = Some("Song Title".to_string());
        assert_eq!(track.display_title(), "Song Title");
    }

    #[test]
    fn display_title_ignores_blank_title() {
        let mut track = sample_track();
        track.title = Some("   ".to_string());
        assert_eq!(track.display_title(), "01 Song.flac");
    }

    #[test]
    fn effective_album_artist_falls_back_to_artist() {
        let track = sample_track();
        assert_eq!(track.effective_album_artist(), Some("Some Artist"));
    }

    #[test]
    fn effective_album_artist_prefers_album_artist() {
        let mut track = sample_track();
        track.album_artist = Some("Various Artists".to_string());
        assert_eq!(track.effective_album_artist(), Some("Various Artists"));
    }

    #[test]
    fn track_round_trips_through_json() {
        let track = sample_track();
        let json = serde_json::to_string(&track).expect("serialize");
        let back: Track = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(track, back);
    }
}
