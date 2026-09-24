//! Unit tests for the lazy shuffle bag (no audio backend involved).

use std::path::PathBuf;

use rand::SeedableRng;
use rand::rngs::StdRng;

use super::*;

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

fn source(names: &[&str]) -> ShuffleSource {
    ShuffleSource::with_rng(paths(names), StdRng::seed_from_u64(7))
}

#[test]
fn every_track_plays_once_before_the_bag_is_exhausted() {
    let mut s = source(&["a", "b", "c", "d", "e"]);
    let mut played = Vec::new();
    while let Some(path) = s.advance() {
        played.push(path);
    }
    assert_eq!(played.len(), 5);
    played.sort();
    played.dedup();
    assert_eq!(played.len(), 5, "no track may repeat within a cycle");
    assert_eq!(s.advance(), None, "scope ends with repeat off");
}

#[test]
fn upcoming_matches_the_order_tracks_actually_play() {
    let s = source(&["a", "b", "c", "d"]);
    let preview: Vec<PathBuf> = s.upcoming(2).into_iter().map(|(_, path)| path).collect();
    assert_eq!(preview.len(), 2);

    let mut s = s;
    assert_eq!(s.advance(), Some(preview[0].clone()));
    assert_eq!(s.advance(), Some(preview[1].clone()));
}

#[test]
fn previous_walks_back_through_this_sessions_history() {
    let mut s = source(&["a", "b", "c", "d"]);
    let first = s.advance().unwrap();
    let second = s.advance().unwrap();
    assert_ne!(first, second);

    assert_eq!(s.retreat(), Some(first.clone()));
    assert_eq!(s.current(), Some(&first));
    assert_eq!(
        s.advance(),
        Some(second),
        "next replays the forward history"
    );
    assert_eq!(s.retreat(), Some(first));
    assert_eq!(s.retreat(), None, "nothing before the first track");
}

#[test]
fn repeat_all_reshuffles_when_the_bag_is_exhausted() {
    let mut s = source(&["a", "b", "c"]);
    s.set_repeat_mode(RepeatMode::All);
    for _ in 0..3 {
        assert!(s.advance().is_some());
    }
    assert!(
        s.advance().is_some(),
        "repeat-all should start a new cycle instead of stopping"
    );
}

#[test]
fn advance_skip_stops_when_the_bag_is_exhausted() {
    let mut s = source(&["a", "b", "c"]);
    s.set_repeat_mode(RepeatMode::All);
    for _ in 0..3 {
        assert!(s.advance_skip().is_some());
    }
    assert_eq!(
        s.advance_skip(),
        None,
        "skipping must not reshuffle, so all-offline scopes terminate"
    );
}

#[test]
fn jump_to_pulls_an_upcoming_track_and_keeps_history_consistent() {
    let mut s = source(&["a", "b", "c", "d"]);
    s.advance().unwrap();
    let (index, path) = s.upcoming(1).into_iter().next().unwrap();

    assert_eq!(s.jump_to(index), Some(path.clone()));
    assert_eq!(s.current(), Some(&path));
    assert_eq!(s.current_item_index(), Some(index));
}

#[test]
fn remove_drops_an_upcoming_track_from_the_bag() {
    let mut s = source(&["a", "b", "c", "d"]);
    let (index, _) = s.upcoming(1).into_iter().next().unwrap();
    s.remove(index);
    assert!(
        !s.upcoming(10).iter().any(|(i, _)| *i == index),
        "removed track must not be scheduled"
    );
}

#[test]
fn enqueue_appends_to_the_end_of_the_current_cycle() {
    let mut s = source(&["a", "b"]);
    s.enqueue(PathBuf::from("c"));
    assert_eq!(s.len(), 3);

    let mut played = Vec::new();
    while let Some(path) = s.advance() {
        played.push(path);
    }
    assert_eq!(played.last(), Some(&PathBuf::from("c")));
}

#[test]
fn snapshot_round_trips_scope_history_and_bag() {
    let mut s = source(&["a", "b", "c", "d"]);
    let first = s.advance().expect("a first track");
    let second = s.advance().expect("a second track");
    let expected_next = s.upcoming(1)[0].1.clone();

    let mut restored = ShuffleSource::from_snapshot(s.snapshot("scope"));

    assert_eq!(restored.current(), Some(&second));
    assert_eq!(restored.repeat_mode(), s.repeat_mode());
    // Retreat walks the saved history, then advancing replays forward and
    // draws the same next track from the saved bag.
    restored.retreat();
    assert_eq!(restored.current(), Some(&first));
    restored.advance();
    restored.advance();
    assert_eq!(restored.current(), Some(&expected_next));
}
