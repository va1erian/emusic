//! Unit tests for album-grid ordering, identity and tile placeholders.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::catalog::{AlbumMeta, belongs_to, compare};
use super::*;
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

#[test]
fn placeholder_colour_is_stable_per_album_name() {
    assert_eq!(
        tile::placeholder_color("Echoes"),
        tile::placeholder_color("Echoes")
    );
    assert_ne!(
        tile::placeholder_color("Echoes"),
        tile::placeholder_color("Voyage")
    );
}
