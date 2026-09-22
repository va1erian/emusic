//! The playback queue: track list, shuffle order and repeat mode.
//!
//! Pure data/logic, no BASS involved, so it's exercised directly by unit
//! tests without needing any audio backend.

use std::path::{Path, PathBuf};

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

/// Repeat behaviour for the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    /// Repeat the whole queue.
    All,
    /// Repeat the current track indefinitely.
    One,
}

/// Track list plus navigation order (identity or shuffled) and repeat mode.
///
/// Shuffle is a *permutation* fixed at the moment it's enabled (or the queue
/// changes), not a fresh random pick on every `advance()` — that's what makes
/// `retreat()` well-defined while shuffled: it just walks the same
/// permutation backwards.
pub struct Queue {
    items: Vec<PathBuf>,
    /// Permutation of `0..items.len()`; navigation walks this, not `items`
    /// directly.
    order: Vec<usize>,
    /// Index into `order` of the current track, if any.
    pos: Option<usize>,
    shuffle: bool,
    repeat: RepeatMode,
    rng: StdRng,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    /// Creates an empty queue with a randomly seeded shuffle RNG.
    pub fn new() -> Self {
        Self::with_rng(StdRng::from_entropy())
    }

    /// Creates an empty queue with a caller-supplied RNG, for deterministic
    /// tests.
    pub fn with_rng(rng: StdRng) -> Self {
        Self {
            items: Vec::new(),
            order: Vec::new(),
            pos: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            rng,
        }
    }

    /// Replaces the whole queue and points the current position at
    /// `start_index` (an index into `items`, not into shuffle order).
    ///
    /// Returns the path now at the current position, if `items` isn't
    /// empty.
    pub fn replace(&mut self, items: Vec<PathBuf>, start_index: usize) -> Option<PathBuf> {
        self.items = items;
        self.rebuild_order(Some(start_index));
        self.current().cloned()
    }

    /// Appends a track to the end of the queue (and its navigation order).
    pub fn enqueue(&mut self, path: PathBuf) {
        let new_item_index = self.items.len();
        self.items.push(path);
        self.order.push(new_item_index);
        if self.shuffle {
            // Keep already-visited entries (everything up to and including
            // `pos`) stable; shuffle the new track in among the remainder.
            let start = self.pos.map_or(0, |p| p + 1);
            if start < self.order.len() {
                self.order[start..].shuffle(&mut self.rng);
            }
        }
    }

    /// Removes the track at `item_index` (an index into the original list).
    /// Adjusts the navigation order and current position to compensate.
    pub fn remove(&mut self, item_index: usize) {
        if item_index >= self.items.len() {
            return;
        }
        self.items.remove(item_index);

        let order_slot = self.order.iter().position(|&i| i == item_index);
        if let Some(slot) = order_slot {
            self.order.remove(slot);
            if let Some(pos) = self.pos {
                if slot < pos {
                    self.pos = Some(pos - 1);
                } else if slot == pos {
                    // The current track was removed: stay at the same
                    // `order` slot, which now holds what used to be the
                    // next track in navigation order — unless there wasn't
                    // one, in which case there's nothing left to play.
                    let new_len = self.order.len();
                    self.pos = if slot < new_len { Some(slot) } else { None };
                }
                // slot > pos: unaffected.
            }
        }
        for index in &mut self.order {
            if *index > item_index {
                *index -= 1;
            }
        }
    }

    /// Moves the track at `from` (index into the original list) to `to`.
    pub fn move_track(&mut self, from: usize, to: usize) {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return;
        }
        let current = self.current_item_index();
        let item = self.items.remove(from);
        self.items.insert(to, item);

        if self.shuffle {
            // Shift shuffled indices to follow the move, keeping the same
            // relative navigation sequence.
            for index in &mut self.order {
                *index = remap_index_after_move(*index, from, to);
            }
        } else {
            // Unshuffled navigation always follows list order directly.
            self.order = (0..self.items.len()).collect();
        }
        if let Some(item_index) = current {
            let new_current = remap_index_after_move(item_index, from, to);
            self.pos = self.order.iter().position(|&i| i == new_current);
        }
    }

    /// Enables/disables shuffle. Enabling re-permutes everything after the
    /// current track; disabling restores the original (item) order, keeping
    /// the current track current.
    pub fn set_shuffle(&mut self, enabled: bool) {
        if self.shuffle == enabled {
            return;
        }
        self.shuffle = enabled;
        let current_item = self.current_item_index();
        self.rebuild_order(current_item);
    }

    pub fn shuffle_enabled(&self) -> bool {
        self.shuffle
    }

    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat
    }

    /// The path at the current position, if any.
    pub fn current(&self) -> Option<&PathBuf> {
        self.current_item_index().map(|i| &self.items[i])
    }

    /// Index into the original list of the current track.
    pub fn current_item_index(&self) -> Option<usize> {
        self.pos.map(|p| self.order[p])
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Iterates the queue in navigation (shuffled, if enabled) order.
    pub fn iter_order(&self) -> impl Iterator<Item = &Path> {
        self.order.iter().map(|&i| self.items[i].as_path())
    }

    /// Advances to the next track per the current repeat mode, updating and
    /// returning the new current path. `None` means playback should stop
    /// (ran off the end with repeat off).
    ///
    /// Does not special-case [`RepeatMode::One`] — that's a "replay the same
    /// track" decision the caller makes at track-end, not a queue navigation
    /// concern (a manual "skip to next" should always advance).
    pub fn advance(&mut self) -> Option<PathBuf> {
        if self.order.is_empty() {
            return None;
        }
        let pos = self.pos?;
        if pos + 1 < self.order.len() {
            self.pos = Some(pos + 1);
        } else {
            match self.repeat {
                RepeatMode::Off => {
                    return None;
                }
                RepeatMode::One | RepeatMode::All => {
                    self.pos = Some(0);
                }
            }
        }
        self.current().cloned()
    }

    /// Moves to the previous track. `None` means there is nothing before the
    /// current track (repeat off and already at the start).
    pub fn retreat(&mut self) -> Option<PathBuf> {
        if self.order.is_empty() {
            return None;
        }
        let pos = self.pos?;
        if pos > 0 {
            self.pos = Some(pos - 1);
        } else {
            match self.repeat {
                RepeatMode::Off => return None,
                RepeatMode::One | RepeatMode::All => {
                    self.pos = Some(self.order.len() - 1);
                }
            }
        }
        self.current().cloned()
    }

    /// Rebuilds `order` from scratch (identity, or a fresh shuffle if
    /// enabled), pointing `pos` at `keep_item_index` if given and still
    /// present.
    fn rebuild_order(&mut self, keep_item_index: Option<usize>) {
        self.order = (0..self.items.len()).collect();
        if self.shuffle {
            if let Some(keep) = keep_item_index {
                // Shuffle everything, then move the kept item to the front
                // so it stays "current" without a jarring reshuffle-under-
                // your-feet feeling.
                self.order.shuffle(&mut self.rng);
                if let Some(slot) = self.order.iter().position(|&i| i == keep) {
                    self.order.swap(0, slot);
                }
            } else {
                self.order.shuffle(&mut self.rng);
            }
        }
        self.pos = match keep_item_index {
            Some(keep) if keep < self.items.len() => self.order.iter().position(|&i| i == keep),
            Some(_) | None => {
                if self.items.is_empty() {
                    None
                } else {
                    Some(0)
                }
            }
        };
    }
}

/// How an item index shifts when the item at `from` is moved to `to`
/// (both indices into the pre-move list).
fn remap_index_after_move(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < to && index > from && index <= to {
        index - 1
    } else if to < from && index >= to && index < from {
        index + 1
    } else {
        index
    }
}

#[cfg(test)]
mod tests {
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
}
