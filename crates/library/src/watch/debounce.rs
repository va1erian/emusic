//! Simple time-based debouncer for file-system notify events.
//!
//! Paths are accumulated in a set; once no new path has arrived for the
//! configured delay, the whole batch is released. This turns the noisy
//! stream of events from a bulk copy or rename into a single scan request.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Debounces a stream of paths into batches.
#[derive(Debug)]
pub struct Debounce {
    delay: Duration,
    pending: HashSet<PathBuf>,
    deadline: Option<Instant>,
}

impl Debounce {
    /// Creates a debouncer that waits `delay` after the last received path
    /// before releasing a batch.
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            pending: HashSet::new(),
            deadline: None,
        }
    }

    /// Adds a path to the pending batch.
    pub fn push(&mut self, path: PathBuf) {
        self.pending.insert(path);
        self.deadline = Some(Instant::now() + self.delay);
    }

    /// Returns whether a batch is ready to be emitted.
    pub fn is_ready(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|deadline| now >= deadline)
    }

    /// Takes the current batch and resets the debouncer.
    ///
    /// Returns an empty vector when nothing is pending.
    pub fn take(&mut self) -> Vec<PathBuf> {
        self.deadline = None;
        self.pending.drain().collect()
    }

    /// Returns whether no paths are pending.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_debounce_yields_nothing() {
        let debounce = Debounce::new(Duration::from_secs(2));
        assert!(!debounce.is_ready(Instant::now()));
    }

    #[test]
    fn batch_released_after_delay() {
        let mut debounce = Debounce::new(Duration::from_millis(50));
        debounce.push(PathBuf::from(r"C:\music\a.flac"));
        debounce.push(PathBuf::from(r"C:\music\b.flac"));

        assert!(!debounce.is_ready(Instant::now()));
        std::thread::sleep(Duration::from_millis(60));
        assert!(debounce.is_ready(Instant::now()));

        let batch = debounce.take();
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn new_path_extends_deadline() {
        let mut debounce = Debounce::new(Duration::from_millis(100));
        debounce.push(PathBuf::from(r"C:\music\a.flac"));
        std::thread::sleep(Duration::from_millis(60));
        debounce.push(PathBuf::from(r"C:\music\b.flac"));

        // Still within the extended window.
        assert!(!debounce.is_ready(Instant::now()));
        std::thread::sleep(Duration::from_millis(110));
        assert!(debounce.is_ready(Instant::now()));
    }

    #[test]
    fn duplicate_paths_are_deduplicated() {
        let mut debounce = Debounce::new(Duration::from_millis(10));
        debounce.push(PathBuf::from(r"C:\music\a.flac"));
        debounce.push(PathBuf::from(r"C:\music\a.flac"));
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(debounce.take().len(), 1);
    }
}
