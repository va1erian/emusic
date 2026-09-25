//! Per-frame context and command sink for view-model updates (#98).
//!
//! Every view model follows the same shape: a plain struct plus a `…Msg`
//! enum, with `update(msg, cx, out)` applying one intent. [`Ctx`] carries the
//! read-only data the model needs that the parent owns (the tracks currently
//! on screen, the playing track), and [`Commands`] collects the one-shot
//! [`Command`]s the model emits, so a model never touches `AppState` and can
//! be unit-tested on its own.

use crate::library_api::{LibraryDataSource, TrackInfo};
use crate::state::Command;

/// Read-only, per-frame context handed to a view model's `update`.
///
/// The parent has already applied its filters, so `tracks` is exactly what
/// the view is showing, in library order; a model's row indices refer to this
/// slice. Anything the UI must agree on but that no single model owns
/// belongs here.
pub struct Ctx<'a> {
    /// The tracks the view is currently showing, in library order.
    pub tracks: &'a [&'a TrackInfo],
    /// Library id of the currently playing track, if any.
    pub playing_id: Option<u64>,
    /// The whole library snapshot, when a model needs to reach beyond
    /// `tracks` (e.g. the album grid rebuilding its album list). `None` for
    /// models that only work from `tracks`.
    pub library: Option<&'a dyn LibraryDataSource>,
}

impl<'a> Ctx<'a> {
    /// Builds the context for a view driven only by `tracks`.
    pub fn new(tracks: &'a [&'a TrackInfo], playing_id: Option<u64>) -> Self {
        Self {
            tracks,
            playing_id,
            library: None,
        }
    }

    /// Builds the context with a library snapshot, for a view that rebuilds
    /// its own lists from the whole library.
    pub fn with_library(
        tracks: &'a [&'a TrackInfo],
        playing_id: Option<u64>,
        library: &'a dyn LibraryDataSource,
    ) -> Self {
        Self {
            tracks,
            playing_id,
            library: Some(library),
        }
    }
}

/// Sink for the [`Command`]s a view model emits while handling a message.
///
/// The frontend drains this after drawing and folds the commands into the
/// shell's pending queue, so the same model works under any toolkit.
#[derive(Debug, Default)]
pub struct Commands {
    queue: Vec<Command>,
}

impl Commands {
    /// An empty sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues one command.
    pub fn push(&mut self, command: Command) {
        self.queue.push(command);
    }

    /// Queues a [`Command::PlayTrack`] from a clicked id and its surrounding
    /// display order (#134); see [`Command::play_track`].
    pub fn play_track(&mut self, id: u64, context: impl IntoIterator<Item = u64>) {
        self.queue.push(Command::play_track(id, context));
    }

    /// Whether no commands have been queued.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Takes the queued commands.
    pub fn into_vec(self) -> Vec<Command> {
        self.queue
    }
}
