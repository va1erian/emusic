//! Background worker that answers search queries against a precomputed
//! index without blocking the UI thread.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use emusic_core::TrackId;
use emusic_search::matcher::PreparedQuery;
use emusic_search::parse;

use crate::library_api::TrackInfo;

use super::index::LibraryIndex;

/// A message sent to the worker thread.
enum Request {
    /// A fresh track snapshot to fold into a new index; sent only when the
    /// track set actually changes.
    Reindex(Vec<TrackInfo>),
    /// Run `text` against the current index. `generation` lets the UI
    /// thread discard answers to queries it has since superseded.
    Query { generation: u64, text: String },
}

/// A finished search's matching track ids.
struct Answer {
    generation: u64,
    ids: Vec<TrackId>,
}

/// Live, non-blocking full-text search over the library's tracks.
///
/// Owns a background thread that keeps the latest [`LibraryIndex`] and
/// matches query text against it. The UI thread only ever sends small
/// messages (a query string, or a fresh track snapshot when the library
/// changes) and polls for results once per frame; the matching work - the
/// only part whose cost scales with library size - never runs on the UI
/// thread, so typing stays responsive even at 100k tracks.
pub struct SearchEngine {
    request_tx: Sender<Request>,
    response_rx: Receiver<Answer>,
    generation: u64,
    last_signature: (usize, Option<u64>, Option<u64>),
    last_query: String,
    /// `None` means "no query active", i.e. every track matches.
    matches: Option<Arc<HashSet<TrackId>>>,
    pending: bool,
}

impl SearchEngine {
    /// Spawns the background worker and returns an engine with no active
    /// query.
    #[must_use]
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<Request>();
        let (response_tx, response_rx) = mpsc::channel::<Answer>();
        thread::Builder::new()
            .name("emusic-search".into())
            .spawn(move || worker_loop(&request_rx, &response_tx))
            .expect("spawn search worker thread");

        Self {
            request_tx,
            response_rx,
            generation: 0,
            last_signature: (0, None, None),
            last_query: String::new(),
            matches: None,
            pending: false,
        }
    }

    /// Feeds this frame's track list and active query text: sends work to
    /// the background thread when either changed since the last call, and
    /// drains any answers that arrived since then.
    pub fn tick(&mut self, tracks: &[TrackInfo], query_text: &str) {
        let signature = LibraryIndex::signature(tracks);
        let mut requery = false;
        if signature != self.last_signature {
            self.last_signature = signature;
            // Ignoring a full channel here just means the previous
            // snapshot is used one frame longer; the next tick retries.
            let _ = self.request_tx.send(Request::Reindex(tracks.to_vec()));
            requery = !query_text.is_empty();
        }

        if query_text.is_empty() {
            self.last_query.clear();
            self.matches = None;
            self.pending = false;
        } else if query_text != self.last_query || requery {
            self.last_query = query_text.to_string();
            self.generation += 1;
            self.pending = true;
            let _ = self.request_tx.send(Request::Query {
                generation: self.generation,
                text: query_text.to_string(),
            });
        }

        while let Ok(answer) = self.response_rx.try_recv() {
            if answer.generation == self.generation {
                self.matches = Some(Arc::new(answer.ids.into_iter().collect()));
                self.pending = false;
            }
            // Older generations were superseded by a later keystroke; drop them.
        }
    }

    /// Whether `track_id` matches the last completed search, or `true` when
    /// no query is active.
    #[must_use]
    pub fn is_match(&self, track_id: u64) -> bool {
        match &self.matches {
            None => true,
            Some(ids) => ids.contains(&TrackId(track_id as i64)),
        }
    }

    /// The number of tracks the last completed search matched, or `None`
    /// when no query is active.
    #[must_use]
    pub fn match_count(&self) -> Option<usize> {
        self.matches.as_ref().map(|ids| ids.len())
    }

    /// Whether a query is active (the box isn't empty), independent of
    /// whether the background worker has answered it yet.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.last_query.is_empty()
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// The worker's main loop: rebuilds the index on `Reindex`, matches on
/// `Query`. Runs until the UI-side sender is dropped.
fn worker_loop(requests: &Receiver<Request>, responses: &Sender<Answer>) {
    let mut current = LibraryIndex::build(&[]);
    while let Ok(request) = requests.recv() {
        match request {
            Request::Reindex(tracks) => current = LibraryIndex::build(&tracks),
            Request::Query { generation, text } => {
                let query = parse(&text);
                let prepared = PreparedQuery::new(&query);
                let stats = |id: TrackId| current.play_counts.get(&id).copied().unwrap_or(0);
                let ids = current
                    .index
                    .search_prepared(&prepared, &stats, &current.order);
                if responses.send(Answer { generation, ids }).is_err() {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn track(id: u64, title: &str, artist: &str) -> TrackInfo {
        TrackInfo {
            id,
            title: title.to_string(),
            artist: artist.to_string(),
            ..Default::default()
        }
    }

    /// Ticks `engine` until it stops being pending or a deadline passes,
    /// polling like the UI thread would once per frame.
    fn settle(engine: &mut SearchEngine, tracks: &[TrackInfo], query: &str) {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            engine.tick(tracks, query);
            if !engine.pending || Instant::now() > deadline {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn empty_query_matches_everything_without_asking_the_worker() {
        let tracks = vec![track(1, "Song A", "Artist A")];
        let mut engine = SearchEngine::new();
        engine.tick(&tracks, "");

        assert!(engine.is_match(1));
        assert!(!engine.is_active());
        assert_eq!(engine.match_count(), None);
    }

    #[test]
    fn query_narrows_to_matching_tracks() {
        let tracks = vec![
            track(1, "Homework", "Daft Punk"),
            track(2, "OK Computer", "Radiohead"),
        ];
        let mut engine = SearchEngine::new();
        settle(&mut engine, &tracks, "radiohead");

        assert!(engine.is_active());
        assert!(!engine.is_match(1));
        assert!(engine.is_match(2));
        assert_eq!(engine.match_count(), Some(1));
    }

    #[test]
    fn a_later_query_supersedes_an_earlier_ones_stale_answer() {
        let tracks = vec![track(1, "Homework", "Daft Punk")];
        let mut engine = SearchEngine::new();
        // Two ticks in the same "frame batch": only the second's generation
        // should ever win, even if the worker answers both.
        engine.tick(&tracks, "daft");
        settle(&mut engine, &tracks, "nonexistent artist");

        assert_eq!(engine.match_count(), Some(0));
    }
}
