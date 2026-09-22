//! The database writer thread: batched upserts, move detection, deletions.
//!
//! The orchestrator streams batches of scanned tracks to this thread so
//! database commits overlap with tag reading. It also owns move detection:
//! when a file vanishes at its old path and an identical one (same size,
//! duration and identifying tags) appears elsewhere, the existing row is
//! re-pointed to the new path instead of delete + insert, which would
//! cascade-delete its play history.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use tracing::warn;

use emusic_core::Track;

use crate::error::Result;
use crate::store::Store;

/// A scanned track on its way to the database.
pub(crate) struct ScannedTrack {
    pub track: Track,
    /// The stored path of the row this file previously occupied (its
    /// normalised key matched an existing row), if any. Set for changed
    /// files, so their row is updated in place rather than re-inserted.
    pub prior_path: Option<PathBuf>,
}

/// Messages from the scan orchestrator to the writer thread.
///
/// [`WriterMessage::Vanished`] is always sent before any
/// [`WriterMessage::Tracks`], so move candidates are known by the time
/// batches arrive.
pub(crate) enum WriterMessage {
    /// Stored rows whose files were not seen on disk, restricted to rows
    /// under roots that were walked completely. Eligible for move matching;
    /// rows still unmatched at [`WriterMessage::Finish`] are deleted when
    /// `delete_remaining` allows it.
    Vanished(Vec<PathBuf>),

    /// A batch of scanned tracks to persist.
    Tracks(Vec<ScannedTrack>),

    /// No more tracks; delete unmatched vanished rows (only when the scan
    /// ran to completion) and exit.
    Finish { delete_remaining: bool },
}

/// Database outcomes counted by the writer.
#[derive(Debug, Default)]
pub(crate) struct WriterStats {
    pub added: u64,
    pub updated: u64,
    pub moved: u64,
    pub deleted: u64,
}

/// Runs the writer loop until [`WriterMessage::Finish`] arrives.
///
/// Owns the store for the duration of the scan; the orchestrator must not
/// touch it until this thread is joined.
pub(crate) fn run(store: &mut Store, inbox: Receiver<WriterMessage>) -> Result<WriterStats> {
    let mut stats = WriterStats::default();
    let mut move_candidates: Vec<Track> = Vec::new();

    for message in inbox {
        match message {
            WriterMessage::Vanished(paths) => {
                move_candidates = load_rows(store, &paths)?;
            }
            WriterMessage::Tracks(batch) => {
                persist_batch(store, batch, &mut move_candidates, &mut stats)?;
            }
            WriterMessage::Finish { delete_remaining } => {
                if delete_remaining && !move_candidates.is_empty() {
                    let paths: Vec<PathBuf> = move_candidates
                        .iter()
                        .map(|track| track.path.clone())
                        .collect();
                    stats.deleted += store.delete_tracks_by_paths(&paths)? as u64;
                }
                return Ok(stats);
            }
        }
    }
    Ok(stats)
}

/// Loads the full rows of vanished files for move matching.
fn load_rows(store: &Store, paths: &[PathBuf]) -> Result<Vec<Track>> {
    let mut rows = Vec::with_capacity(paths.len());
    for path in paths {
        match store.get_track_by_path(path)? {
            Some(track) => rows.push(track),
            None => {
                warn!(path = %path.display(), "vanished row disappeared before move matching")
            }
        }
    }
    Ok(rows)
}

/// Persists one batch: changed files and detected moves re-point their
/// existing row (preserving id, `added_at` and play history); the rest are
/// upserted in a single transaction.
fn persist_batch(
    store: &mut Store,
    batch: Vec<ScannedTrack>,
    move_candidates: &mut Vec<Track>,
    stats: &mut WriterStats,
) -> Result<()> {
    let mut fresh: Vec<Track> = Vec::new();
    for item in batch {
        let ScannedTrack {
            mut track,
            prior_path,
        } = item;

        if let Some(prior) = prior_path {
            // A changed file: update its row in place, which also refreshes
            // the stored path spelling to the one seen on this walk.
            if store.move_track(&prior, &mut track)? {
                stats.updated += 1;
                continue;
            }
            // The row vanished concurrently; fall through to a plain insert.
        }

        if let Some(index) = find_move(move_candidates, &track) {
            let old = move_candidates.swap_remove(index);
            if store.move_track(&old.path, &mut track)? {
                stats.moved += 1;
            } else {
                warn!(path = %old.path.display(), "move candidate vanished before update");
                fresh.push(track);
            }
            continue;
        }

        fresh.push(track);
    }

    if !fresh.is_empty() {
        let count = fresh.len() as u64;
        store.upsert_tracks(&mut fresh)?;
        stats.added += count;
    }
    Ok(())
}

/// Finds a vanished row that is very likely the same track at a new path:
/// same size, duration and identifying tags.
fn find_move(move_candidates: &[Track], track: &Track) -> Option<usize> {
    move_candidates.iter().position(|candidate| {
        candidate.size == track.size
            && candidate.duration_ms == track.duration_ms
            && candidate.title == track.title
            && candidate.artist == track.artist
            && candidate.album == track.album
            && candidate.track_no == track.track_no
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_track(path: &str, title: &str, size: u64, duration_ms: u32) -> Track {
        Track {
            id: emusic_core::TrackId::UNASSIGNED,
            path: PathBuf::from(path),
            dir: PathBuf::from(r"C:\music"),
            filename: "a.flac".to_string(),
            ext: "flac".to_string(),
            size,
            mtime: 1_700_000_000,
            kind: emusic_core::TrackKind::Stream,
            duration_ms,
            bitrate: None,
            sample_rate: None,
            channels: None,
            title: Some(title.to_string()),
            artist: Some("Artist".to_string()),
            album_artist: None,
            album: Some("Album".to_string()),
            genre: None,
            year: None,
            track_no: Some(1),
            disc_no: None,
            composer: None,
            comment: None,
            art_source: emusic_core::ArtSource::None,
            added_at: 1_700_000_000,
        }
    }

    #[test]
    fn find_move_requires_matching_fingerprint() {
        let candidates = vec![
            sample_track(r"C:\music\a.flac", "A", 100, 60_000),
            sample_track(r"C:\music\b.flac", "B", 200, 90_000),
        ];
        let track = sample_track(r"C:\elsewhere\b.flac", "B", 200, 90_000);
        assert_eq!(find_move(&candidates, &track), Some(1));

        let wrong_size = sample_track(r"C:\elsewhere\b.flac", "B", 999, 90_000);
        assert_eq!(find_move(&candidates, &wrong_size), None);

        let wrong_title = sample_track(r"C:\elsewhere\b.flac", "C", 200, 90_000);
        assert_eq!(find_move(&candidates, &wrong_title), None);
    }
}
