//! Unit tests for the column browser's selection model and facet lists.

use std::collections::BTreeSet;

use crate::library_api::TrackInfo;

use super::selection::PaneSelection;
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
fn empty_selection_means_all() {
    let mut selection = PaneSelection::default();
    assert!(selection.is_all());
    assert!(selection.matches("anything"));

    selection.click(Some("Rock"), false);
    assert!(!selection.is_all());
    assert!(selection.matches("Rock"));
    assert!(!selection.matches("Jazz"));
}

#[test]
fn plain_click_replaces_and_ctrl_click_toggles() {
    let mut selection = PaneSelection::default();
    selection.click(Some("Rock"), false);
    selection.click(Some("Jazz"), false);
    assert_eq!(selection.len(), 1);
    assert!(selection.contains("Jazz"));
    assert!(!selection.contains("Rock"));

    // Ctrl adds a second value...
    selection.click(Some("Rock"), true);
    assert_eq!(selection.len(), 2);

    // ...and ctrl-clicking an existing value removes it again.
    selection.click(Some("Rock"), true);
    assert_eq!(selection.len(), 1);
    assert!(selection.contains("Jazz"));
}

#[test]
fn all_row_clears_the_selection() {
    let mut selection = PaneSelection::default();
    selection.click(Some("Rock"), true);
    selection.click(Some("Jazz"), true);
    assert_eq!(selection.len(), 2);

    selection.click(None, false);
    assert!(selection.is_all());
}

#[test]
fn retain_drops_values_the_parent_no_longer_offers() {
    let mut selection = PaneSelection::default();
    selection.click(Some("Rock"), false);
    selection.click(Some("Jazz"), true);

    selection.retain(&available(&["Jazz"]));
    assert!(selection.contains("Jazz"));
    assert!(!selection.contains("Rock"));
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
