//! [`Command`]: the one-shot requests panels/views emit during `ui()` for the
//! shell to apply afterwards.

use std::path::PathBuf;

use super::{Accent, PanelKind, View};

/// One-shot request emitted by a panel/view during `ui()`, applied by the
/// shell after layout so widgets never need `&mut AppState` themselves.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    SetView(View),
    ToggleTheme,
    SetAccent(Accent),
    TogglePanel(PanelKind),
    /// Show/hide the Music view's column browser (#16).
    ToggleColumnBrowser,
    SetSearchQuery(String),
    PlayerPlayPause,
    PlayerStop,
    PlayerNext,
    PlayerPrevious,
    PlayerSeek(std::time::Duration),
    PlayerSetVolume(f32),
    PlayerToggleRepeat,
    PlayerToggleShuffle,
    /// Start playing this track. A stand-in for real queue control (#4):
    /// currently a no-op in the shell, kept here so the track table's
    /// double-click/Enter/context menu have somewhere to send intent.
    PlayTrack(u64),
    /// Play a whole album (#17): replaces the queue with these tracks, in
    /// order, and starts at the first. The ids are resolved to paths by the
    /// shell, same as [`Command::PlayTrack`].
    PlayAlbum(Vec<u64>),
    /// Start a lazy shuffled playback over these tracks (#57), showing
    /// `label` as the active scope. The ids are resolved to paths by the
    /// shell, same as [`Command::PlayTrack`].
    ShuffleScope {
        ids: Vec<u64>,
        label: String,
    },
    /// "Play next" from a track's context menu; same caveat as
    /// [`Command::PlayTrack`].
    PlayTrackNext(u64),
    /// "Add to queue" from a track's context menu; same caveat as
    /// [`Command::PlayTrack`].
    QueueTrack(u64),
    /// Jump to a queue entry by its current index and start playback.
    PlayerQueueJump(usize),
    /// Remove a queue entry by its current index.
    PlayerQueueRemove(usize),
    /// Add a root folder to the library (Settings → Library / empty state).
    LibraryAddFolder(PathBuf),
    /// Remove a root folder from the library.
    LibraryRemoveFolder(PathBuf),
    /// Rescan every enabled library folder on demand.
    LibraryRescan,
    /// Stop the scan currently running, if any.
    LibraryCancelScan,
    /// Remove one playback history entry (History view, #24).
    HistoryRemove(i64),
    /// Clear the whole playback history (History view, #24), after the
    /// confirmation dialog.
    HistoryClear,
}
