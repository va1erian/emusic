//! Album identity, ordering and track matching for the album grid (#17).
//!
//! Kept separate from the view layout so the view module stays focused on
//! widgets and the ordering/track rules can be unit-tested directly.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::models::{AlbumKey, AlbumSort};
use crate::library_api::{AlbumInfo, LibraryDataSource, TrackInfo};

/// Per-album metadata derived from the track list: where to read cover art
/// from and the newest track id, used to approximate "recently added" until
/// the store exposes real timestamps.
pub struct AlbumMeta {
    pub art_path: String,
    pub latest_track_id: u64,
}

/// Builds the per-album art/recency index in a single pass over the tracks.
pub fn album_meta(library: &dyn LibraryDataSource) -> HashMap<AlbumKey, AlbumMeta> {
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
pub fn sorted_albums<'a>(
    library: &'a dyn LibraryDataSource,
    meta: &HashMap<AlbumKey, AlbumMeta>,
    sort: AlbumSort,
) -> Vec<&'a AlbumInfo> {
    let mut albums: Vec<&AlbumInfo> = library.albums().iter().collect();
    albums.sort_by(|a, b| compare(a, b, sort, meta));
    albums
}

/// Orders two albums for a given sort mode.
pub fn compare(
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
pub fn album_tracks<'a>(
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
pub fn belongs_to(track: &TrackInfo, album: &AlbumInfo) -> bool {
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

#[cfg(test)]
mod tests {
    //! Unit tests for album-grid ordering and identity, moved with the code (#93).

    use std::cmp::Ordering;
    use std::collections::HashMap;

    use super::super::models::{AlbumKey, AlbumSort};
    use super::{AlbumMeta, belongs_to, compare};
    use crate::library_api::{AlbumInfo, TrackInfo};

    fn album(name: &str, artist: &str, year: Option<u32>) -> AlbumInfo {
        AlbumInfo {
            name: name.to_string(),
            artist: artist.to_string(),
            year,
            track_count: 1,
        }
    }

    fn meta(entries: &[(&str, &str, u64)]) -> HashMap<AlbumKey, AlbumMeta> {
        entries
            .iter()
            .map(|(name, artist, id)| {
                (
                    AlbumKey {
                        name: (*name).to_string(),
                        artist: (*artist).to_string(),
                    },
                    AlbumMeta {
                        art_path: String::new(),
                        latest_track_id: *id,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn artist_sort_is_case_insensitive_and_ties_break_on_name() {
        let lower = album("Zebra", "alpha", Some(2000));
        let upper = album("Apple", "Beta", Some(1990));
        assert_eq!(
            compare(&lower, &upper, AlbumSort::Artist, &HashMap::new()),
            Ordering::Less
        );

        let first = album("Apple", "Same", None);
        let second = album("Banana", "Same", None);
        assert_eq!(
            compare(&first, &second, AlbumSort::Artist, &HashMap::new()),
            Ordering::Less
        );
    }

    #[test]
    fn album_sort_orders_by_title_then_artist() {
        let a = album("Alpha", "Zed", None);
        let b = album("Beta", "Ann", None);
        assert_eq!(
            compare(&a, &b, AlbumSort::Album, &HashMap::new()),
            Ordering::Less
        );
    }

    #[test]
    fn year_sort_puts_newest_first_and_unknown_last() {
        let new = album("A", "a", Some(2020));
        let old = album("B", "b", Some(1990));
        let unknown = album("C", "c", None);

        assert_eq!(
            compare(&new, &old, AlbumSort::Year, &HashMap::new()),
            Ordering::Less
        );
        assert_eq!(
            compare(&old, &unknown, AlbumSort::Year, &HashMap::new()),
            Ordering::Less
        );
    }

    #[test]
    fn recently_added_sort_uses_latest_track_id() {
        let a = album("A", "a", None);
        let b = album("B", "b", None);
        let meta = meta(&[("A", "a", 10), ("B", "b", 42)]);

        assert_eq!(
            compare(&a, &b, AlbumSort::RecentlyAdded, &meta),
            Ordering::Greater
        );
    }

    #[test]
    fn track_album_key_is_skipped_when_artist_is_missing() {
        let mut track = TrackInfo {
            album: "Some Album".to_string(),
            artist: "Some Artist".to_string(),
            ..TrackInfo::default()
        };
        assert_eq!(
            AlbumKey::of_track(&track),
            Some(AlbumKey {
                name: "Some Album".to_string(),
                artist: "Some Artist".to_string(),
            })
        );

        track.artist.clear();
        assert!(AlbumKey::of_track(&track).is_none());
    }

    #[test]
    fn tracks_match_an_album_by_name_and_artist_or_missing_artist() {
        let target = album("Record", "Artist", None);
        let matching = TrackInfo {
            album: "Record".to_string(),
            artist: "Artist".to_string(),
            ..TrackInfo::default()
        };
        let untagged = TrackInfo {
            album: "Record".to_string(),
            ..TrackInfo::default()
        };
        let other_album = TrackInfo {
            album: "Other".to_string(),
            artist: "Artist".to_string(),
            ..TrackInfo::default()
        };

        assert!(belongs_to(&matching, &target));
        assert!(belongs_to(&untagged, &target));
        assert!(!belongs_to(&other_album, &target));
    }
}
