//! Unit tests for the queue navigation logic (shuffle/repeat/remove/move).

use super::*;

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

fn seeded_queue() -> Queue {
    Queue::with_rng(StdRng::seed_from_u64(42))
}

#[test]
fn replace_sets_current_to_start_index() {
    let mut q = seeded_queue();
    let current = q.replace(paths(&["a", "b", "c"]), 1);
    assert_eq!(current, Some(PathBuf::from("b")));
    assert_eq!(q.current(), Some(&PathBuf::from("b")));
}

#[test]
fn next_walks_in_order_without_repeat() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c"]), 0);
    assert_eq!(q.advance(), Some(PathBuf::from("b")));
    assert_eq!(q.advance(), Some(PathBuf::from("c")));
    assert_eq!(q.advance(), None, "should stop at the end with repeat off");
}

#[test]
fn next_wraps_with_repeat_all() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c"]), 0);
    q.set_repeat_mode(RepeatMode::All);
    q.advance();
    q.advance();
    assert_eq!(
        q.advance(),
        Some(PathBuf::from("a")),
        "should wrap to start"
    );
}

#[test]
fn previous_walks_backwards_without_repeat() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c"]), 2);
    assert_eq!(q.retreat(), Some(PathBuf::from("b")));
    assert_eq!(q.retreat(), Some(PathBuf::from("a")));
    assert_eq!(q.retreat(), None);
}

#[test]
fn previous_wraps_with_repeat_all() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c"]), 0);
    q.set_repeat_mode(RepeatMode::All);
    assert_eq!(q.retreat(), Some(PathBuf::from("c")));
}

#[test]
fn shuffle_keeps_current_track_current() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c", "d", "e"]), 2);
    q.set_shuffle(true);
    assert_eq!(
        q.current(),
        Some(&PathBuf::from("c")),
        "enabling shuffle must not change the currently playing track"
    );
}

#[test]
fn shuffle_then_next_then_previous_returns_to_the_same_track() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c", "d", "e"]), 0);
    q.set_shuffle(true);
    let first = q.current().cloned();
    let second = q.advance();
    assert_ne!(
        first, second,
        "sanity: shuffle order should differ (seeded)"
    );
    let back = q.retreat();
    assert_eq!(
        back, first,
        "previous() must walk the same permutation backwards"
    );
}

#[test]
fn disabling_shuffle_restores_original_order() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c"]), 0);
    q.set_shuffle(true);
    q.set_shuffle(false);
    assert_eq!(
        q.iter_order().collect::<Vec<_>>(),
        vec![Path::new("a"), Path::new("b"), Path::new("c"),]
    );
}

#[test]
fn remove_current_track_moves_to_the_next_one() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c"]), 1);
    q.remove(1);
    assert_eq!(q.current(), Some(&PathBuf::from("c")));
}

#[test]
fn remove_last_current_track_clears_position() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b"]), 1);
    q.remove(1);
    assert_eq!(q.current(), None);
    assert_eq!(q.len(), 1);
}

#[test]
fn remove_before_current_shifts_position_back() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c"]), 2);
    q.remove(0);
    assert_eq!(q.current(), Some(&PathBuf::from("c")));
}

#[test]
fn move_track_keeps_the_same_track_current() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c", "d"]), 3);
    q.move_track(0, 3);
    assert_eq!(q.current(), Some(&PathBuf::from("d")));
    assert_eq!(
        q.iter_order().collect::<Vec<_>>(),
        vec![
            Path::new("b"),
            Path::new("c"),
            Path::new("d"),
            Path::new("a"),
        ]
    );
}

#[test]
fn enqueue_adds_to_the_end_of_navigation_order_when_not_shuffled() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b"]), 0);
    q.enqueue(PathBuf::from("c"));
    assert_eq!(q.advance(), Some(PathBuf::from("b")));
    assert_eq!(q.advance(), Some(PathBuf::from("c")));
}

#[test]
fn snapshot_round_trips_items_order_and_position() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c", "d"]), 2);
    q.set_shuffle(true);

    let restored = Queue::from_snapshot(q.snapshot());

    assert_eq!(restored.current(), q.current());
    assert_eq!(
        restored.iter_order().collect::<Vec<_>>(),
        q.iter_order().collect::<Vec<_>>(),
        "the shuffled navigation order must survive the round-trip"
    );
}

#[test]
fn from_snapshot_repairs_an_invalid_permutation_and_position() {
    let mut q = seeded_queue();
    q.replace(paths(&["a", "b", "c"]), 0);
    let mut snapshot = q.snapshot();
    snapshot.order = vec![0, 0, 99];
    snapshot.pos = Some(9);

    let repaired = Queue::from_snapshot(snapshot);

    assert_eq!(
        repaired.iter_order().collect::<Vec<_>>(),
        vec![Path::new("a"), Path::new("b"), Path::new("c")],
        "an invalid permutation falls back to list order"
    );
    assert!(repaired.current().is_none(), "an invalid pos is dropped");
}
