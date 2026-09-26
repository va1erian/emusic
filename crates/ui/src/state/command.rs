//! [`Command`]: the one-shot requests panels/views emit during `ui()` for the
//! shell to apply afterwards.

use std::path::PathBuf;

use emusic_player::tracker::TrackerSettings;

use crate::library_api::{EditRequest, TrackQuery};

use super::{Accent, PanelKind, SettingsTab, View, VizCommand};

/// One-shot request emitted by a panel/view during `ui()`, applied by the
/// shell after layout so widgets never need `&mut AppState` themselves.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    SetView(View),
    /// Switch to the Settings view with `tab` selected (#364). Unlike
    /// [`Command::SetView`], which leaves the active Settings sub-page where it
    /// was, this also selects the tab (e.g. Help → About emusic).
    OpenSettings(SettingsTab),
    ToggleTheme,
    SetAccent(Accent),
    /// Tint the win32 window's acrylic bands with the accent, or stop (#355).
    SetAccentTint(bool),
    /// Set the accent tint strength, `0..=255` (#355).
    SetAccentTintStrength(u8),
    /// Cycle the visualizer strip's mode (spectrum → oscilloscope → off),
    /// emitted by clicking the strip (#25).
    CycleVisualizer,
    /// Show, hide, move or drive the projectM visualization (#300).
    Viz(VizCommand),
    TogglePanel(PanelKind),
    /// Show/hide the Music view's column browser (#16).
    ToggleColumnBrowser,
    SetSearchQuery(String),
    /// Jump to the Artists view, filtered to `name` (seeding the top-bar
    /// search box, matching the global search popup). Emitted by clicking an
    /// artist name in the now-playing surfaces.
    GoToArtist(String),
    /// Jump to the Albums view with the named album selected, so its tracks
    /// are shown. Emitted by clicking an album name in the now-playing
    /// surfaces.
    GoToAlbum {
        name: String,
        artist: String,
    },
    PlayerPlayPause,
    PlayerStop,
    PlayerNext,
    PlayerPrevious,
    PlayerSeek(std::time::Duration),
    PlayerSetVolume(f32),
    PlayerToggleRepeat,
    PlayerToggleShuffle,
    /// Start playing `id`, replacing the queue with `context` (the ids of
    /// the view it was clicked from, in that view's current visible/sorted
    /// order) and starting at `id`'s position within it (#134). An empty
    /// `context` falls back to a one-track queue.
    ///
    /// Build this with [`Command::play_track`] rather than the variant
    /// directly, so a bare id — and the one-entry queue that made
    /// Next/Previous act like Stop (#134) — isn't the easy path for a
    /// future call site.
    PlayTrack {
        id: u64,
        context: Vec<u64>,
    },
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
    /// Flip whether the track is starred (favorited), from a row's star
    /// column or Star/Unstar context-menu entry (#131).
    ToggleStarred(u64),
    /// Open the single-track tag editor for `id` (#172), seeded from the
    /// track's current tags. The shell resolves the id against the library.
    OpenTagEditor(u64),
    /// Submit a batch of tag edits to the library backend (#172). The dialog
    /// builds these from the user's form; the backend applies them off the UI
    /// thread and reports the outcomes through
    /// [`LibraryDataSource::take_tag_edit_results`].
    ///
    /// [`LibraryDataSource::take_tag_edit_results`]:
    ///     crate::library_api::LibraryDataSource::take_tag_edit_results
    RequestTagEdits(Vec<EditRequest>),
    /// Run an online metadata lookup for the track at `path`, seeded from
    /// `query`, to fill the tag editor (#208). The backend does the lookup off
    /// the UI thread and reports the candidates through
    /// [`LibraryDataSource::take_auto_tag_results`].
    ///
    /// [`LibraryDataSource::take_auto_tag_results`]:
    ///     crate::library_api::LibraryDataSource::take_auto_tag_results
    AutoTagTrack {
        /// The file to look up.
        path: PathBuf,
        /// The local metadata the lookup is seeded from.
        query: TrackQuery,
    },
    /// Stop the online auto-tag lookup currently in flight, if any (#210).
    CancelAutoTag,
    /// Replace the tracker module playback settings (interpolation, ramping,
    /// emulation, ...), applied live to the player and persisted.
    SetTrackerSettings(TrackerSettings),
    /// Set (or clear, with `None`) the soundfont MIDI files play with,
    /// applied live to the player and persisted.
    SetMidiSoundfont(Option<PathBuf>),
    /// Set (or clear, with `None`) the HVSC Songlengths database path (#192),
    /// applied to the player and persisted.
    SetSonglengthsPath(Option<PathBuf>),
    /// Set the SID fallback play length, in seconds (#192), applied to the
    /// player and persisted.
    SetSidFallbackSecs(u32),
}

impl Command {
    /// Builds a [`Command::PlayTrack`] from a clicked track's id and the
    /// surrounding order it was clicked from (#134) — e.g. a view's current
    /// filtered/sorted track ids. This is the only intended way to construct
    /// `PlayTrack`: it keeps "pass the whole list" the path of least
    /// resistance instead of a bare id that silently empties the queue.
    pub fn play_track(id: u64, context: impl IntoIterator<Item = u64>) -> Self {
        Command::PlayTrack {
            id,
            context: context.into_iter().collect(),
        }
    }
}
