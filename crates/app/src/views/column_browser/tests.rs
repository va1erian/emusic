//! Unit tests for the column browser's facet lists and cascading state.
//! The selection-model tests moved to `emusic-ui` with the code in #93.

use std::collections::BTreeSet;

use crate::library_api::TrackInfo;

use super::{ColumnBrowserState, entries_for, values};

fn track(genre: &str, artist: &str, album: &str) -> TrackInfo {
    TrackInfo {
        genre: genre.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        ..TrackInfo::default()
    }
}

fn available(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|v| (*v).to_string()).collect()
}

#[test]
fn entries_start_with_all_and_count_each_facet() {
    let a = track("Rock", "A", "First");
    let b = track("Rock", "B", "Second");
    let c = track("Jazz", "A", "First");
    let tracks = [&a, &b, &c];

    let entries = entries_for(&tracks, |t| &t.genre);
    assert_eq!(entries[0].value, None);
    assert_eq!(entries[0].label(), "All");
    assert_eq!(entries[0].count, 3);

    let genres: Vec<_> = entries[1..].iter().map(|e| e.label()).collect();
    assert_eq!(genres, ["Jazz", "Rock"]);
    assert_eq!(entries[1].count, 1);
    assert_eq!(entries[2].count, 2);
}

#[test]
fn entries_are_sorted_case_insensitively_and_show_unknown() {
    let a = track("Rock", "banana", "X");
    let b = track("Rock", "Apple", "X");
    let c = track("Rock", "", "X");
    let tracks = [&a, &b, &c];

    let entries = entries_for(&tracks, |t| &t.artist);
    let artists: Vec<_> = entries[1..].iter().map(|e| e.label()).collect();
    assert_eq!(artists, ["(unknown)", "Apple", "banana"]);
}

#[test]
fn matches_cascades_across_all_three_panes() {
    let mut state = ColumnBrowserState::default();
    assert!(state.matches(&track("Rock", "A", "First")));

    state.genres.click(Some("Rock"), false);
    assert!(state.matches(&track("Rock", "A", "First")));
    assert!(!state.matches(&track("Jazz", "A", "First")));

    state.artists.click(Some("A"), false);
    assert!(state.matches(&track("Rock", "A", "First")));
    assert!(!state.matches(&track("Rock", "B", "First")));

    state.albums.click(Some("First"), false);
    assert!(state.matches(&track("Rock", "A", "First")));
    assert!(!state.matches(&track("Rock", "A", "Second")));
}

#[test]
fn values_collects_every_facet_except_all() {
    let a = track("Rock", "A", "First");
    let entries = entries_for(&[&a], |t| &t.genre);
    assert_eq!(values(&entries), available(&["Rock"]));
}
