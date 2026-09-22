//! Builds an [`emusic_search::Index`] from the shell's [`TrackInfo`] rows.
//!
//! `emusic-search`'s [`Haystack`](emusic_search::matcher::haystack::Haystack)
//! is built from `emusic_core::Track`, which is richer than the UI-facing
//! [`TrackInfo`] (it has no `composer`/`comment`/`album_artist` tags). We
//! convert losslessly for every field `TrackInfo` actually carries; the
//! fields it doesn't carry simply never match, which is harmless since the
//! UI never exposed them either.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use emusic_core::{ArtSource, Track, TrackId, TrackKind};
use emusic_search::Index;

use crate::library_api::TrackInfo;

/// A ready-to-query snapshot of the library: the folded-text index, a
/// track-id-ordered candidate list (mirrors [`TrackInfo`] order so results
/// stay stable), and play counts for the `plays:` field (kept outside the
/// index since `Track` itself has no play-count field).
pub(crate) struct LibraryIndex {
    pub index: Index,
    pub order: Vec<TrackId>,
    pub play_counts: HashMap<TrackId, u64>,
}

impl LibraryIndex {
    /// Builds a fresh index from every track. Folds each track's text
    /// fields once; callers should only do this when the track set changes,
    /// not on every keystroke.
    pub fn build(tracks: &[TrackInfo]) -> Self {
        let mut index = Index::new();
        let mut order = Vec::with_capacity(tracks.len());
        let mut play_counts = HashMap::with_capacity(tracks.len());

        for info in tracks {
            let id = TrackId(info.id as i64);
            index.add_track(to_search_track(info, id));
            order.push(id);
            play_counts.insert(id, u64::from(info.play_count));
        }

        Self {
            index,
            order,
            play_counts,
        }
    }

    /// Cheap signature used to decide whether the track set changed enough
    /// to warrant a rebuild: length plus the first/last id. Good enough to
    /// catch scans finishing or folders being removed without hashing the
    /// whole library every frame.
    pub fn signature(tracks: &[TrackInfo]) -> (usize, Option<u64>, Option<u64>) {
        (
            tracks.len(),
            tracks.first().map(|t| t.id),
            tracks.last().map(|t| t.id),
        )
    }
}

/// Converts a [`TrackInfo`] into the `emusic_core::Track` shape the matcher
/// expects. `path`/`dir`/`filename`/`ext` are derived from `TrackInfo::path`
/// since the UI type only keeps the joined string.
fn to_search_track(info: &TrackInfo, id: TrackId) -> Track {
    let path = PathBuf::from(&info.path);
    let dir = path.parent().map(PathBuf::from).unwrap_or_default();
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    Track {
        id,
        path,
        dir,
        filename,
        ext: info.format.clone(),
        size: 0,
        mtime: 0,
        kind: TrackKind::Stream,
        duration_ms: info.duration.as_millis() as u32,
        bitrate: info.bitrate,
        sample_rate: info.sample_rate,
        channels: info.channels,
        title: non_empty(&info.title),
        artist: non_empty(&info.artist),
        album_artist: None,
        album: non_empty(&info.album),
        genre: non_empty(&info.genre),
        year: info.year.map(|y| y as i32),
        track_no: info.track_no,
        disc_no: info.disc_no,
        composer: None,
        comment: None,
        art_source: ArtSource::None,
        added_at: UNIX_EPOCH
            .elapsed()
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emusic_search::matcher::PreparedQuery;
    use emusic_search::parse;

    fn track(id: u64, title: &str, artist: &str, play_count: u32) -> TrackInfo {
        TrackInfo {
            id,
            title: title.to_string(),
            artist: artist.to_string(),
            path: format!("C:/music/{title}.mp3"),
            play_count,
            ..Default::default()
        }
    }

    #[test]
    fn build_indexes_every_track_and_its_play_count() {
        let tracks = vec![
            track(1, "Song A", "Artist A", 5),
            track(2, "Song B", "Artist B", 9),
        ];
        let index = LibraryIndex::build(&tracks);

        assert_eq!(index.index.len(), 2);
        assert_eq!(index.order, vec![TrackId(1), TrackId(2)]);
        assert_eq!(index.play_counts[&TrackId(2)], 9);
    }

    #[test]
    fn search_matches_a_field_scoped_query() {
        let tracks = vec![
            track(1, "Homework", "Daft Punk", 0),
            track(2, "Discovery", "Daft Punk", 0),
            track(3, "OK Computer", "Radiohead", 0),
        ];
        let index = LibraryIndex::build(&tracks);
        let query = parse("artist:radiohead");
        let prepared = PreparedQuery::new(&query);
        let stats = |id: TrackId| index.play_counts.get(&id).copied().unwrap_or(0);

        let matches = index.index.search_prepared(&prepared, &stats, &index.order);

        assert_eq!(matches, vec![TrackId(3)]);
    }

    #[test]
    fn signature_changes_when_the_track_set_changes() {
        let one = vec![track(1, "Song A", "Artist A", 0)];
        let two = vec![
            track(1, "Song A", "Artist A", 0),
            track(2, "Song B", "Artist B", 0),
        ];
        assert_ne!(LibraryIndex::signature(&one), LibraryIndex::signature(&two));
    }
}
