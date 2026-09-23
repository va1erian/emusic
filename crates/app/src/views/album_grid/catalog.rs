//! Album identity, ordering and track matching for the album grid (#17).
//!
//! Kept separate from the view layout so the view module stays focused on
//! widgets and the ordering/track rules can be unit-tested directly.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::{AlbumKey, AlbumSort};
use crate::library_api::{AlbumInfo, LibraryDataSource, TrackInfo};

/// Per-album metadata derived from the track list: where to read cover art
/// from and the newest track id, used to approximate "recently added" until
/// the store exposes real timestamps.
pub(super) struct AlbumMeta {
    pub(super) art_path: String,
    pub(super) latest_track_id: u64,
}

/// Builds the per-album art/recency index in a single pass over the tracks.
pub(super) fn album_meta(library: &dyn LibraryDataSource) -> HashMap<AlbumKey, AlbumMeta> {
    let mut meta = HashMap::new();
    for track in library.tracks() {
        let Some(key) = AlbumKey::of_track(track) else {
            continue;
        };
        let entry = meta.entry(key).or_insert_with(|| AlbumMeta {
            art_path: track.path.clone(),
            latest_track_id: track.id,
        });
        entry.latest_track_id = entry.latest_track_id.max(track.id);
    }
    meta
}

/// Albums in the given sort order.
pub(super) fn sorted_albums<'a>(
    library: &'a dyn LibraryDataSource,
    meta: &HashMap<AlbumKey, AlbumMeta>,
    sort: AlbumSort,
) -> Vec<&'a AlbumInfo> {
    let mut albums: Vec<&AlbumInfo> = library.albums().iter().collect();
    albums.sort_by(|a, b| compare(a, b, sort, meta));
    albums
}

/// Orders two albums for a given sort mode.
pub(super) fn compare(
    a: &AlbumInfo,
    b: &AlbumInfo,
    sort: AlbumSort,
    meta: &HashMap<AlbumKey, AlbumMeta>,
) -> Ordering {
    fn by_artist(a: &AlbumInfo, b: &AlbumInfo) -> Ordering {
        ci_cmp(&a.artist, &b.artist).then_with(|| ci_cmp(&a.name, &b.name))
    }

    match sort {
        AlbumSort::Artist => by_artist(a, b),
        AlbumSort::Album => ci_cmp(&a.name, &b.name).then_with(|| ci_cmp(&a.artist, &b.artist)),
        AlbumSort::Year => b.year.cmp(&a.year).then_with(|| by_artist(a, b)),
        AlbumSort::RecentlyAdded => {
            let a_id = meta.get(&AlbumKey::of(a)).map_or(0, |m| m.latest_track_id);
            let b_id = meta.get(&AlbumKey::of(b)).map_or(0, |m| m.latest_track_id);
            b_id.cmp(&a_id).then_with(|| by_artist(a, b))
        }
    }
}

/// Tracks belonging to `album`, in disc/track order.
pub(super) fn album_tracks<'a>(
    library: &'a dyn LibraryDataSource,
    album: &AlbumInfo,
) -> Vec<&'a TrackInfo> {
    let mut tracks: Vec<&TrackInfo> = library
        .tracks()
        .iter()
        .filter(|track| belongs_to(track, album))
        .collect();
    tracks.sort_by(|a, b| {
        a.disc_no
            .cmp(&b.disc_no)
            .then_with(|| a.track_no.cmp(&b.track_no))
            .then_with(|| a.title.cmp(&b.title))
    });
    tracks
}

/// Whether `track` belongs to `album`: its album tag matches and its artist
/// tag matches, or is empty (how missing tags surface in the mock data).
pub(super) fn belongs_to(track: &TrackInfo, album: &AlbumInfo) -> bool {
    track.album == album.name && (track.artist == album.artist || track.artist.is_empty())
}

/// Case-insensitive comparison that lowercases lazily instead of allocating,
/// since it runs O(n log n) times per sort (every frame).
fn ci_cmp(a: &str, b: &str) -> Ordering {
    fn lower(s: &str) -> impl Iterator<Item = char> + '_ {
        s.chars().flat_map(char::to_lowercase)
    }
    lower(a).cmp(lower(b))
}
