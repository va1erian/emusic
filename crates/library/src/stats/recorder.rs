//! Background recording of play/skip events.
//!
//! Resolving a path to a [`TrackId`](emusic_core::TrackId) and writing the
//! `plays`/`track_stats` rows both touch the database, so [`StatsRecorder`]
//! does this off the caller's thread (the UI/player thread) on a small
//! writer thread of its own, mirroring the scanner's writer (see
//! `crate::scanner`).

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use emusic_core::PlayEvent;
use tracing::warn;

use crate::store::Store;

/// A completed or skipped playback, ready to be recorded.
///
/// This is intentionally not `emusic_player::PlayerEvent::PlayFinished`:
/// this crate has no dependency on `emusic-player` (see the module docs),
/// so the app crate builds one of these from that event plus the timestamp
/// it recorded when the track started.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayRecord {
    /// The file path that was played, as reported by the player.
    pub path: PathBuf,
    /// Unix timestamp (seconds, UTC) when playback of this track started.
    pub started_at: i64,
    /// How much of the track was actually played, in milliseconds.
    pub listened_ms: u32,
    /// Whether the player's completion threshold (its configurable default
    /// is `min(50%, 4 min)`) was met.
    pub completed: bool,
}

/// Messages sent to the writer thread.
enum Message {
    Record(PlayRecord),
    /// Blocks the sender until every message queued before it has been
    /// processed. Used by tests (and callers that need a synchronisation
    /// point, e.g. before reading stats back) rather than for normal use.
    Flush(Sender<()>),
}

/// Records [`PlayRecord`]s to the library store on a background thread.
///
/// Dropping the recorder closes the channel and joins the writer thread,
/// so any records already sent are flushed before the drop returns.
pub struct StatsRecorder {
    /// `None` only after `drop` has taken it, to close the channel and let
    /// the writer thread's receive loop end before it is joined.
    sender: Option<Sender<Message>>,
    handle: Option<JoinHandle<()>>,
}

impl StatsRecorder {
    /// Spawns the writer thread. `store` is shared with whatever else needs
    /// to read or write the same library database (e.g. the UI thread doing
    /// stats queries), guarded by the mutex — see [`Store`]'s docs on why it
    /// isn't `Sync` on its own.
    pub fn spawn(store: Arc<Mutex<Store>>) -> Self {
        let (sender, inbox) = mpsc::channel::<Message>();
        let handle = thread::spawn(move || {
            for message in inbox {
                match message {
                    Message::Record(record) => {
                        if let Err(error) = process(&store, &record) {
                            warn!(path = %record.path.display(), %error, "failed to record play");
                        }
                    }
                    Message::Flush(ack) => {
                        let _ = ack.send(());
                    }
                }
            }
        });
        Self {
            sender: Some(sender),
            handle: Some(handle),
        }
    }

    /// Queues a record for writing. Never blocks; failures (e.g. an unknown
    /// path) are logged on the writer thread and otherwise swallowed, since
    /// there is no meaningful way for playback to react to a stats-write
    /// failure.
    pub fn record(&self, record: PlayRecord) {
        // The only way this send fails is if the writer thread panicked and
        // the channel disconnected; there is nothing to recover into.
        if let Some(sender) = &self.sender {
            let _ = sender.send(Message::Record(record));
        }
    }

    /// Blocks until every record queued before this call has been written
    /// (or failed and been logged). Intended for tests and orderly shutdown.
    pub fn flush(&self) {
        let Some(sender) = &self.sender else {
            return;
        };
        let (ack_tx, ack_rx) = mpsc::channel();
        if sender.send(Message::Flush(ack_tx)).is_ok() {
            let _ = ack_rx.recv();
        }
    }
}

impl Drop for StatsRecorder {
    fn drop(&mut self) {
        // Drop the sender first so the writer thread's receive loop ends;
        // otherwise the join below would block forever.
        self.sender.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Resolves `record`'s path and writes it to the store.
fn process(store: &Mutex<Store>, record: &PlayRecord) -> crate::error::Result<()> {
    // The lock is only held for the resolve + write, not while queued.
    let mut store = store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(track_id) = store.resolve_track_id(&record.path)? else {
        warn!(path = %record.path.display(), "no library track for played path, dropping record");
        return Ok(());
    };
    let event = PlayEvent::new(
        track_id,
        record.started_at,
        record.listened_ms,
        record.completed,
    );
    store.record_play(&event)
}

#[cfg(test)]
mod tests;
