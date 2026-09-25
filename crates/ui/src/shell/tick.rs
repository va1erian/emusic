//! What one [`Shell::tick`](super::Shell::tick) produced (#97): the changes
//! since the previous tick and when the frontend should wake next.

use std::time::Duration;

use bitflags::bitflags;

bitflags! {
    /// What changed between two ticks, for retained-mode frontends. An
    /// immediate-mode frontend (egui) ignores it and redraws.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Changes: u32 {
        /// The library snapshot, scan progress or history changed.
        const LIBRARY = 1 << 0;
        /// A different track became (or stopped being) now playing.
        const NOW_PLAYING = 1 << 1;
        /// The playback position advanced.
        const POSITION = 1 << 2;
        /// The upcoming queue changed.
        const QUEUE = 1 << 3;
        /// Search results (either engine) changed.
        const SEARCH = 1 << 4;
        /// The active view changed.
        const VIEW = 1 << 5;
        /// The theme or accent changed.
        const THEME = 1 << 6;
        /// Panel visibility changed.
        const PANELS = 1 << 7;
        /// The projectM visualization was shown, hidden or moved to another
        /// surface, or its engine status changed (#300).
        const VISUALIZATION = 1 << 8;
    }
}

/// The result of one tick: what changed since the previous tick and when the
/// frontend should wake next (`None` when idle — no continuous repaint, per
/// #6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tick {
    pub changes: Changes,
    pub next_wake: Option<Duration>,
}
