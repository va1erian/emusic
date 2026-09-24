//! Lazy shuffle source: a shuffled "bag" over a scope's track paths.
//!
//! A scope can hold 100k+ tracks on a network drive, so the queue must not
//! materialise a shuffled order up front (or clone the whole scope into the
//! visible queue). Instead the bag hands out one index at a time; only the
//! next few are ever asked for (see [`super::QueueSource::upcoming`]). Once
//! the bag empties it is reshuffled for repeat-all, so every track plays once
//! per cycle. The tracks played this session are kept in `history`, which is
//! what `previous` walks back through.

use std::path::PathBuf;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use super::RepeatMode;
use super::snapshot::{ShuffleSnapshot, sanitize_indices};

/// A lazy, non-repeating shuffle over a fixed scope.
pub struct ShuffleSource {
    /// The scope, in its original order. Indices into this are what the bag
    /// and history store, so they stay stable while the scope is unchanged.
    items: Vec<PathBuf>,
    /// Not-yet-played item indices, popped from the end; the next track is
    /// `bag.last()`.
    bag: Vec<usize>,
    /// Item indices played this session, oldest first. The current track is
    /// the entry at `cursor`.
    history: Vec<usize>,
    /// Position within `history`; `advance` first replays forward through the
    /// existing history before pulling a new track from the bag.
    cursor: usize,
    repeat: RepeatMode,
    rng: StdRng,
}

impl ShuffleSource {
    /// Creates a shuffled source over `items` with a randomly seeded RNG.
    pub fn new(items: Vec<PathBuf>) -> Self {
        Self::with_rng(items, StdRng::from_entropy())
    }

    /// Creates a source with a caller-supplied RNG, for deterministic tests.
    pub fn with_rng(items: Vec<PathBuf>, mut rng: StdRng) -> Self {
        let mut bag: Vec<usize> = (0..items.len()).collect();
        bag.shuffle(&mut rng);
        Self {
            items,
            bag,
            history: Vec::new(),
            cursor: 0,
            repeat: RepeatMode::Off,
            rng,
        }
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    /// Snapshots the scope, its label, played history and remaining bag
    /// (#214), so playback can resume without replaying or reshuffling.
    pub fn snapshot(&self, label: impl Into<String>) -> ShuffleSnapshot {
        ShuffleSnapshot {
            items: self.items.clone(),
            label: label.into(),
            history: self.history.clone(),
            cursor: self.cursor,
            bag: self.bag.clone(),
            repeat: self.repeat,
        }
    }

    /// Rebuilds a scoped shuffle from `snapshot`, dropping any out-of-range
    /// history/bag indices a hand-edited config could contain. The RNG seed
    /// is not part of the snapshot: a restored bag plays the saved order
    /// first, and only a later re-shuffle draws fresh randomness.
    pub fn from_snapshot(snapshot: ShuffleSnapshot) -> Self {
        let items = snapshot.items;
        let len = items.len();
        let history = sanitize_indices(&snapshot.history, len);
        let bag = sanitize_indices(&snapshot.bag, len);
        let cursor = snapshot.cursor.min(history.len().saturating_sub(1));
        Self {
            items,
            bag,
            history,
            cursor,
            repeat: snapshot.repeat,
            rng: StdRng::from_entropy(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// The path at the current position, if any track has been drawn yet.
    pub fn current(&self) -> Option<&PathBuf> {
        self.history.get(self.cursor).map(|&i| &self.items[i])
    }

    /// The scope index of the current track, if any.
    pub fn current_item_index(&self) -> Option<usize> {
        self.history.get(self.cursor).copied()
    }

    /// The next `limit` not-yet-played tracks, in the order they will play,
    /// each paired with its scope index (used by `jump_to`/`remove`).
    pub fn upcoming(&self, limit: usize) -> Vec<(usize, PathBuf)> {
        self.bag
            .iter()
            .rev()
            .take(limit)
            .map(|&i| (i, self.items[i].clone()))
            .collect()
    }

    /// Advances to the next track: replays the forward history first, then
    /// draws from the bag, reshuffling it for repeat-all when exhausted.
    pub fn advance(&mut self) -> Option<PathBuf> {
        if let Some(path) = self.replay_forward() {
            return Some(path);
        }
        self.draw_from_bag(true)
    }

    /// Like [`Self::advance`], but never reshuffles an exhausted bag. Used
    /// when skipping unreadable tracks so an all-offline scope can't loop
    /// forever.
    pub fn advance_skip(&mut self) -> Option<PathBuf> {
        if let Some(path) = self.replay_forward() {
            return Some(path);
        }
        self.draw_from_bag(false)
    }

    /// Moves back one track through this session's history. `None` at the
    /// start of the history.
    pub fn retreat(&mut self) -> Option<PathBuf> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        self.current().cloned()
    }

    /// Jumps directly to an upcoming scope index, dropping any forward
    /// history (the jump becomes the new current track).
    pub fn jump_to(&mut self, item_index: usize) -> Option<PathBuf> {
        let pos = self.bag.iter().position(|&i| i == item_index)?;
        self.bag.remove(pos);
        self.history.truncate(self.cursor + 1);
        self.history.push(item_index);
        self.cursor = self.history.len() - 1;
        self.current().cloned()
    }

    /// Drops an upcoming scope index from the bag (the playing track is left
    /// alone; the player advances past it separately).
    pub fn remove(&mut self, item_index: usize) {
        if let Some(pos) = self.bag.iter().position(|&i| i == item_index) {
            self.bag.remove(pos);
        }
    }

    /// Appends a track to the end of the current cycle.
    pub fn enqueue(&mut self, path: PathBuf) {
        let index = self.items.len();
        self.items.push(path);
        self.bag.insert(0, index);
    }

    /// Inserts a track to play immediately after the current one.
    pub fn play_next(&mut self, path: PathBuf) {
        let index = self.items.len();
        self.items.push(path);
        self.bag.push(index);
    }

    /// Iterates the whole scope in its original order (not play order).
    pub fn iter_scope(&self) -> impl Iterator<Item = &std::path::Path> {
        self.items.iter().map(PathBuf::as_path)
    }

    fn replay_forward(&mut self) -> Option<PathBuf> {
        if self.cursor + 1 < self.history.len() {
            self.cursor += 1;
            self.current().cloned()
        } else {
            None
        }
    }

    fn draw_from_bag(&mut self, reshuffle: bool) -> Option<PathBuf> {
        if self.bag.is_empty() {
            if !reshuffle || self.repeat != RepeatMode::All || self.items.is_empty() {
                return None;
            }
            self.reshuffle();
        }
        let index = self.bag.pop()?;
        // Drawing a fresh track invalidates any forward history.
        self.history.truncate(self.cursor + 1);
        self.history.push(index);
        self.cursor = self.history.len() - 1;
        self.current().cloned()
    }

    /// Starts a new cycle: refill and reshuffle the bag, forgetting the old
    /// history so it can't grow without bound across cycles.
    fn reshuffle(&mut self) {
        self.history.clear();
        self.cursor = 0;
        self.bag = (0..self.items.len()).collect();
        self.bag.shuffle(&mut self.rng);
    }
}

#[cfg(test)]
mod tests;
