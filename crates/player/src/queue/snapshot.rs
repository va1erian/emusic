//! Serializable snapshots of the queue's navigation state (#214).
//!
//! Captured on exit and restored on the next launch, so a session (including
//! its shuffled order and scoped shuffle) resumes exactly where it left off.
//! The [`Queue`] implementation lives here; [`ShuffleSource`]'s lives beside
//! its type in `shuffle.rs`, which owns its private fields.
//!
//! Snapshots come from a `config.toml` that a user could hand-edit, so every
//! index is validated on restore and any out-of-range value falls back to a
//! sensible default instead of panicking.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Queue, RepeatMode};

/// A whole playback queue, tagged by how it produces the next track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueueSnapshot {
    /// A concrete, user-editable list (see [`Queue`]).
    Explicit(ExplicitQueueSnapshot),
    /// A lazy shuffle over a scope (see [`ShuffleSource`](super::ShuffleSource)).
    Shuffle(ShuffleSnapshot),
}

impl QueueSnapshot {
    /// Whether the snapshot carries no tracks, so there is nothing to
    /// restore.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Explicit(snapshot) => snapshot.items.is_empty(),
            Self::Shuffle(snapshot) => snapshot.items.is_empty(),
        }
    }
}

/// Snapshot of an explicit [`Queue`]'s list, navigation order and position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplicitQueueSnapshot {
    /// Track paths in their original (pre-shuffle) order.
    pub items: Vec<PathBuf>,
    /// Permutation of `0..items.len()` navigation walks.
    pub order: Vec<usize>,
    /// Index into `order` of the current track, if any.
    pub pos: Option<usize>,
    /// Whether shuffle is enabled.
    pub shuffle: bool,
    /// Repeat behaviour.
    pub repeat: RepeatMode,
}

/// Snapshot of a scoped shuffle's scope, label, history and remaining bag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShuffleSnapshot {
    /// The scope's paths, in their original order; history/bag index into it.
    pub items: Vec<PathBuf>,
    /// Human-readable label of the scope (e.g. `"Album — Purple Motion"`).
    pub label: String,
    /// Scope indices played this session, oldest first.
    pub history: Vec<usize>,
    /// Position within `history` of the current track.
    pub cursor: usize,
    /// Not-yet-played scope indices, popped from the end.
    pub bag: Vec<usize>,
    /// Repeat behaviour.
    pub repeat: RepeatMode,
}

impl Queue {
    /// Snapshots the queue's list, navigation order and position.
    pub fn snapshot(&self) -> ExplicitQueueSnapshot {
        ExplicitQueueSnapshot {
            items: self.items.clone(),
            order: self.order.clone(),
            pos: self.pos,
            shuffle: self.shuffle,
            repeat: self.repeat,
        }
    }

    /// Rebuilds a queue from `snapshot`, repairing any invalid permutation or
    /// position a hand-edited config could contain.
    pub fn from_snapshot(snapshot: ExplicitQueueSnapshot) -> Self {
        let mut queue = Self::new();
        queue.items = snapshot.items;
        queue.shuffle = snapshot.shuffle;
        queue.repeat = snapshot.repeat;
        queue.order = if is_permutation(&snapshot.order, queue.items.len()) {
            snapshot.order
        } else {
            (0..queue.items.len()).collect()
        };
        queue.pos = snapshot.pos.filter(|&pos| pos < queue.order.len());
        queue
    }
}

/// Whether `order` is a permutation of `0..len`.
pub(super) fn is_permutation(order: &[usize], len: usize) -> bool {
    if order.len() != len {
        return false;
    }
    let mut seen = vec![false; len];
    for &index in order {
        if index >= len || seen[index] {
            return false;
        }
        seen[index] = true;
    }
    true
}

/// Drops indices that are out of range (a hand-edited config), preserving
/// order.
pub(super) fn sanitize_indices(indices: &[usize], len: usize) -> Vec<usize> {
    indices.iter().copied().filter(|&i| i < len).collect()
}
