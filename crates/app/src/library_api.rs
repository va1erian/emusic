//! UI-facing, read-only library data trait.
//!
//! This stands in for `crates/library` (and `emusic-core::Track`) until
//! those land. The shell only reads through [`LibraryDataSource`], so
//! swapping in a real SQLite-backed store later is a matter of implementing
//! this trait, not touching view code.

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
    pub duration: Duration,
    pub path: String,
    /// e.g. "mp3", "flac", "xm", "it" ...
    pub format: String,
    pub play_count: u32,
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
}
