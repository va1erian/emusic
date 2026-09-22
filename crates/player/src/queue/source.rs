//! The player's track source: an explicit, user-editable list, or a lazy
//! shuffle over a scope.
//!
//! Both variants answer the same navigation questions (`current`, `advance`,
//! `retreat`, `upcoming`, ...), so [`crate::Player`] can treat them uniformly.
//! The explicit list keeps the full queue; the shuffle source keeps only the
//! scope and hands tracks out on demand (see [`super::ShuffleSource`]).

use std::path::{Path, PathBuf};

use super::{Queue, RepeatMode, ShuffleSource};

/// Where the player's next track comes from.
pub enum QueueSource {
    /// A concrete, user-editable list (the classic queue).
    Explicit(Queue),
    /// A lazy shuffle over a scope; only a short preview is materialised.
    Shuffle(ShuffleSource),
}

impl QueueSource {
    /// An empty explicit queue.
    pub fn explicit() -> Self {
        Self::Explicit(Queue::new())
    }

    pub fn is_shuffle(&self) -> bool {
        matches!(self, Self::Shuffle(_))
    }

    pub fn current(&self) -> Option<&PathBuf> {
        match self {
            Self::Explicit(queue) => queue.current(),
            Self::Shuffle(source) => source.current(),
        }
    }

    pub fn current_item_index(&self) -> Option<usize> {
        match self {
            Self::Explicit(queue) => queue.current_item_index(),
            Self::Shuffle(source) => source.current_item_index(),
        }
    }

    /// The whole source in navigation order. For the shuffle variant this is
    /// the scope's original order, not the shuffled play order.
    pub fn iter_paths(&self) -> Box<dyn Iterator<Item = &Path> + '_> {
        match self {
            Self::Explicit(queue) => Box::new(queue.iter_order()),
            Self::Shuffle(source) => Box::new(source.iter_scope()),
        }
    }

    /// The upcoming tracks, capped at `limit` so a huge scope never
    /// materialises into the visible queue.
    pub fn upcoming(&self, limit: usize) -> Vec<(usize, PathBuf)> {
        match self {
            Self::Explicit(queue) => queue.upcoming(limit),
            Self::Shuffle(source) => source.upcoming(limit),
        }
    }

    /// Replaces the source with an explicit list starting at `start_index`.
    /// Keeps the explicit queue's shuffle flag when one is already active.
    pub fn replace(&mut self, items: Vec<PathBuf>, start_index: usize) -> Option<PathBuf> {
        match self {
            Self::Explicit(queue) => queue.replace(items, start_index),
            Self::Shuffle(_) => {
                let repeat = self.repeat_mode();
                let mut queue = Queue::new();
                queue.set_repeat_mode(repeat);
                let current = queue.replace(items, start_index);
                *self = Self::Explicit(queue);
                current
            }
        }
    }

    pub fn enqueue(&mut self, path: PathBuf) {
        match self {
            Self::Explicit(queue) => queue.enqueue(path),
            Self::Shuffle(source) => source.enqueue(path),
        }
    }

    /// Inserts `path` to play immediately after the current track.
    pub fn play_next(&mut self, path: PathBuf) {
        match self {
            Self::Explicit(queue) => {
                let current = queue.current_item_index();
                let new_index = queue.len();
                queue.enqueue(path);
                if let Some(current) = current {
                    let target = current + 1;
                    if target < new_index {
                        queue.move_track(new_index, target);
                    }
                }
            }
            Self::Shuffle(source) => source.play_next(path),
        }
    }

    pub fn remove(&mut self, item_index: usize) {
        match self {
            Self::Explicit(queue) => queue.remove(item_index),
            Self::Shuffle(source) => source.remove(item_index),
        }
    }

    pub fn move_track(&mut self, from: usize, to: usize) {
        if let Self::Explicit(queue) = self {
            queue.move_track(from, to);
        }
    }

    pub fn jump_to(&mut self, item_index: usize) -> Option<PathBuf> {
        match self {
            Self::Explicit(queue) => queue.jump_to(item_index),
            Self::Shuffle(source) => source.jump_to(item_index),
        }
    }

    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        match self {
            Self::Explicit(queue) => queue.set_repeat_mode(mode),
            Self::Shuffle(source) => source.set_repeat_mode(mode),
        }
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        match self {
            Self::Explicit(queue) => queue.repeat_mode(),
            Self::Shuffle(source) => source.repeat_mode(),
        }
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        if let Self::Explicit(queue) = self {
            queue.set_shuffle(enabled);
        }
    }

    pub fn shuffle_enabled(&self) -> bool {
        match self {
            Self::Explicit(queue) => queue.shuffle_enabled(),
            Self::Shuffle(_) => true,
        }
    }

    pub fn advance(&mut self) -> Option<PathBuf> {
        match self {
            Self::Explicit(queue) => queue.advance(),
            Self::Shuffle(source) => source.advance(),
        }
    }

    /// Advances without reshuffling an exhausted shuffle bag; used to skip
    /// unreadable tracks without looping forever.
    pub fn advance_skip(&mut self) -> Option<PathBuf> {
        match self {
            Self::Explicit(queue) => queue.advance(),
            Self::Shuffle(source) => source.advance_skip(),
        }
    }

    pub fn retreat(&mut self) -> Option<PathBuf> {
        match self {
            Self::Explicit(queue) => queue.retreat(),
            Self::Shuffle(source) => source.retreat(),
        }
    }
}
