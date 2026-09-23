//! UI-facing, read-only library data trait.
//!
//! This stands in for `crates/library` (and `emusic-core::Track`) until
//! those land. The shell only reads through [`LibraryDataSource`], so
//! swapping in a real SQLite-backed store later is a matter of implementing
//! this trait, not touching view code.

use std::path::PathBuf;
use std::time::Duration;

/// A "most played" time window (all time / last 30 days / last year).
///
/// Re-exported from the library crate so the UI selector and the store's
/// ranking query agree on exactly one definition; the app already depends on
/// `emusic-library`.
pub use emusic_library::stats::StatsWindow;

/// Minimal, local stand-in for `emusic_core::Track`.
#[derive(Debug, Clone, Default)]
pub struct TrackInfo {
    pub id: u64,
    pub title: String,
    pub artist: String,
    /// Tagged album artist, used for grouping compilations; empty when the
    /// tag is absent (the Properties dialog then shows nothing for it).
    pub album_artist: String,
    pub album: String,
    pub genre: String,
    pub track_no: Option<u32>,
    /// Release year, when known (usually inherited from the album).
    pub year: Option<u32>,
    pub disc_no: Option<u32>,
    /// Tagged composer, empty when absent.
    pub composer: String,
    /// Free-form tagged comment, empty when absent.
    pub comment: String,
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
    /// Whether the user has starred (favorited) this track (#131).
    pub starred: bool,
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

/// A genre with its track count, mirroring [`ArtistInfo`]/[`AlbumInfo`].
#[derive(Debug, Clone, Default)]
pub struct GenreInfo {
    pub name: String,
    pub track_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct FolderInfo {
    pub path: String,
    pub track_count: usize,
}

/// A node in the Folders view's collapsible directory tree (#18), built from
/// the library index's directory grouping.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DirNodeInfo {
    /// Full path of this directory.
    pub path: String,
    /// Display name (the last path component, or the drive root).
    pub name: String,
    /// Tracks located directly in this directory, not counting children.
    pub direct_track_count: usize,
    /// Tracks in this directory and all descendants.
    pub total_track_count: usize,
    /// Child directories, sorted by name.
    pub children: Vec<DirNodeInfo>,
}

/// One row in the play history list: a single recorded play of a track.
///
/// History is per *play*, not per track (the same track can appear several
/// times), so it carries the played track's identity for double-click
/// playback plus the play's own timing and outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Identifies one `plays` row, for removing that entry.
    pub id: i64,
    /// Library id of the track that was played, for double-click playback.
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    /// Unix timestamp (seconds, UTC) when playback started.
    pub played_at: i64,
    /// How much of the track was actually played, in milliseconds.
    pub played_ms: u32,
    /// Whether the player's completion threshold was met.
    pub completed: bool,
}

/// Read-only view over the music library, as needed by the shell's views.
pub trait LibraryDataSource {
    fn tracks(&self) -> &[TrackInfo];
    fn albums(&self) -> &[AlbumInfo];
    fn artists(&self) -> &[ArtistInfo];
    fn genres(&self) -> &[GenreInfo];
    fn folders(&self) -> &[FolderInfo];
    /// Root nodes of the library's directory tree (Folders view, #18).
    fn dir_tree(&self) -> &[DirNodeInfo];
    /// Recorded plays, newest first.
    fn history(&self) -> &[HistoryEntry];
    /// Tracks ordered by descending play count within `window`.
    fn most_played(&self, window: StatsWindow) -> &[TrackInfo];

    /// Starred (favorited) tracks, in library order (#131).
    ///
    /// Defaults to filtering [`LibraryDataSource::tracks`] by
    /// [`TrackInfo::starred`]; backends with a dedicated starred set can
    /// override it.
    fn starred_tracks(&self) -> Vec<&TrackInfo> {
        self.tracks().iter().filter(|track| track.starred).collect()
    }

    /// Removes one playback history entry by its [`HistoryEntry::id`].
    ///
    /// A no-op if no such entry exists (e.g. it was already removed by
    /// another path).
    fn remove_history_entry(&mut self, _id: i64) {}

    /// Removes every playback history entry.
    fn clear_history(&mut self) {}

    /// Sets whether the track with `id` is starred (#131). A no-op for
    /// backends that don't persist a library.
    fn set_starred(&mut self, _id: u64, _starred: bool) {}

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
