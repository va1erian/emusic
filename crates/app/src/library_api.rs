//! UI-facing, read-only library data trait.
//!
//! This stands in for `crates/library` (and `emusic-core::Track`) until
//! those land. The shell only reads through [`LibraryDataSource`], so
//! swapping in a real SQLite-backed store later is a matter of implementing
//! this trait, not touching view code.

use std::path::PathBuf;
use std::time::Duration;

/// Minimal, local stand-in for `emusic_core::Track`.
#[derive(Debug, Clone, Default)]
pub struct TrackInfo {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub track_no: Option<u32>,
    /// Release year, when known (usually inherited from the album).
    pub year: Option<u32>,
    pub disc_no: Option<u32>,
    pub duration: Duration,
    pub path: String,
    /// e.g. "mp3", "flac", "xm", "it" ...
    pub format: String,
    /// Human-readable codec, e.g. "MP3", "FLAC", "MPEG-1 Layer III".
    pub codec: String,
    /// Bitrate in kbps, when known.
    pub bitrate: Option<u32>,
    /// Sample rate in Hz, when known.
    pub sample_rate: Option<u32>,
    /// Bit depth in bits, when known.
    pub bit_depth: Option<u8>,
    /// Channel count, when known.
    pub channels: Option<u8>,
    pub play_count: u32,
    /// Minutes elapsed since the track was last played; `None` if it has
    /// never been played. Kept as a plain number (rather than a formatted
    /// string or wall-clock timestamp) so it can be sorted numerically and
    /// formatted on demand with [`format_minutes_ago`].
    pub last_played_minutes_ago: Option<u32>,
}

/// Formats an elapsed-minutes value into a short, human-readable string,
/// e.g. `"45 min ago"`, `"3 h ago"`, `"2 d ago"`.
pub fn format_minutes_ago(minutes: u32) -> String {
    if minutes < 60 {
        format!("{minutes} min ago")
    } else if minutes < 1440 {
        format!("{} h ago", minutes / 60)
    } else {
        format!("{} d ago", minutes / 1440)
    }
}

#[derive(Debug, Clone, Default)]
pub struct AlbumInfo {
    pub name: String,
    pub artist: String,
    pub year: Option<u32>,
    pub track_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ArtistInfo {
    pub name: String,
    pub track_count: usize,
    pub album_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct FolderInfo {
    pub path: String,
    pub track_count: usize,
}

/// One row in the play history list.
#[derive(Debug, Clone, Default)]
pub struct HistoryEntry {
    pub track_title: String,
    pub artist: String,
    /// Human-readable, already formatted (mock data has no wall clock
    /// dependency, so this stays a plain string rather than a timestamp).
    pub played_at: String,
}

/// Read-only view over the music library, as needed by the shell's views.
pub trait LibraryDataSource {
    fn tracks(&self) -> &[TrackInfo];
    fn albums(&self) -> &[AlbumInfo];
    fn artists(&self) -> &[ArtistInfo];
    fn genres(&self) -> &[String];
    fn folders(&self) -> &[FolderInfo];
    fn history(&self) -> &[HistoryEntry];
    /// Tracks ordered by descending play count.
    fn most_played(&self) -> &[TrackInfo];

    fn track_count(&self) -> usize {
        self.tracks().len()
    }

    fn total_duration(&self) -> Duration {
        self.tracks().iter().map(|t| t.duration).sum()
    }

    /// Looks up a track by its full file path.
    fn track_by_path(&self, path: &str) -> Option<&TrackInfo> {
        self.tracks().iter().find(|t| t.path == path)
    }

    /// Applies any background updates (new index snapshots, scan progress,
    /// play-recorded stats) that arrived since the last frame. Real backends
    /// override this; mock backends have nothing to do.
    fn tick(&mut self) {}

    /// Sets the watched library folders. Real backends start background
    /// loading, scanning and watching, and purge tracks under folders that
    /// were removed; mock backends ignore this.
    fn set_folders(&mut self, _folders: &[PathBuf]) {}

    /// Rescans every enabled folder, even if the folder set is unchanged.
    fn rescan(&mut self) {}

    /// Requests cancellation of the scan currently running, if any.
    fn cancel_scan(&mut self) {}

    /// Whether a background scan is currently running (drives the status
    /// bar's progress line and cancel button).
    fn is_scanning(&self) -> bool {
        false
    }

    /// Returns an optional status line (e.g. scan progress) to show in the
    /// status bar alongside the player state.
    fn status_text(&self) -> Option<String> {
        None
    }
}
