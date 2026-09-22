//! Background worker for the folder watcher.
//!
//! Owns the `notify` [`RecommendedWatcher`] and runs the scheduling loop
//! that debounces local events and polls remote roots.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use notify::Watcher as NotifyWatcher;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode};
use tracing::{debug, warn};

use super::{Debounce, WatchError, WatchEvent, WatchOptions, is_remote_root};
use crate::scanner::paths::normalize_key;

enum Command {
    SetRoots(Vec<PathBuf>),
    Shutdown,
}

/// Manages file-system watches and remote-root polling for library folders.
pub struct Watcher {
    command_tx: Sender<Command>,
    handle: Option<JoinHandle<()>>,
}

impl Watcher {
    /// Starts a background watcher with the given options and event sink.
    ///
    /// The watcher initially has no roots; call [`Watcher::set_roots`] to
    /// begin watching.
    pub fn new(options: WatchOptions, events: Sender<WatchEvent>) -> Result<Self, WatchError> {
        let (command_tx, command_rx) = mpsc::channel();
        let (notify_tx, notify_rx) = mpsc::channel();

        let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| match result {
                Ok(event) => {
                    debug!(kind = ?event.kind, paths = ?event.paths, "notify event");
                    for path in event.paths {
                        let _ = notify_tx.send(path);
                    }
                }
                Err(error) => {
                    warn!(%error, "notify error");
                }
            },
            Config::default(),
        )?;

        let handle = thread::spawn(move || {
            run_loop(&mut watcher, command_rx, notify_rx, options, events);
        });

        Ok(Self {
            command_tx,
            handle: Some(handle),
        })
    }

    /// Replaces the set of watched roots. Local roots are registered with
    /// `notify`; remote roots are polled instead.
    pub fn set_roots(&self, roots: Vec<PathBuf>) -> Result<(), WatchError> {
        self.command_tx
            .send(Command::SetRoots(roots))
            .map_err(|_| WatchError::ThreadGone)
    }

    /// Stops the background thread and waits for it to finish.
    pub fn shutdown(mut self) -> Result<(), WatchError> {
        let _ = self.command_tx.send(Command::Shutdown);
        if let Some(handle) = self.handle.take() {
            handle.join().map_err(|_| WatchError::ThreadPanic)?;
        }
        Ok(())
    }
}

fn run_loop(
    watcher: &mut RecommendedWatcher,
    command_rx: Receiver<Command>,
    notify_rx: Receiver<PathBuf>,
    options: WatchOptions,
    events: Sender<WatchEvent>,
) {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut debounce = Debounce::new(options.debounce);
    let mut next_remote_poll: HashMap<PathBuf, Instant> = HashMap::new();
    let now = Instant::now();

    loop {
        // Drain commands first so a shutdown is never delayed by events.
        match command_rx.try_recv() {
            Ok(Command::SetRoots(new_roots)) => {
                update_watched_roots(watcher, &roots, &new_roots);
                next_remote_poll.retain(|root, _| new_roots.iter().any(|r| paths_equal(r, root)));
                for root in &new_roots {
                    if is_remote_root(root) {
                        next_remote_poll.entry(root.clone()).or_insert(now);
                    }
                }
                roots = new_roots;
            }
            Ok(Command::Shutdown) => {
                emit_batch(&events, &mut debounce);
                return;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }

        // Drain notify events, mapping each path back to its watched root so
        // deletions are detected by scanning the root rather than the missing
        // file.
        while let Ok(path) = notify_rx.try_recv() {
            if let Some(root) = root_for_path(&roots, &path) {
                debounce.push(root);
            }
        }

        let now = Instant::now();

        // Emit debounced batch when ready.
        if debounce.is_ready(now) {
            emit_batch(&events, &mut debounce);
        }

        // Poll remote roots whose interval has elapsed.
        let mut remote_paths: Vec<PathBuf> = Vec::new();
        for root in &roots {
            if !is_remote_root(root) {
                continue;
            }
            let deadline = next_remote_poll
                .get(root)
                .copied()
                .unwrap_or(now + options.remote_poll_interval);
            if now >= deadline {
                remote_paths.push(root.clone());
                next_remote_poll.insert(root.clone(), now + options.remote_poll_interval);
            }
        }
        if !remote_paths.is_empty() {
            let _ = events.send(WatchEvent::ScanRequested {
                paths: remote_paths,
            });
        }

        thread::sleep(options.tick_interval);
    }
}

fn emit_batch(events: &Sender<WatchEvent>, debounce: &mut Debounce) {
    let paths: Vec<PathBuf> = debounce.take();
    if !paths.is_empty() {
        let _ = events.send(WatchEvent::ScanRequested { paths });
    }
}

fn update_watched_roots(
    watcher: &mut RecommendedWatcher,
    old_roots: &[PathBuf],
    new_roots: &[PathBuf],
) {
    for root in old_roots {
        if !new_roots.iter().any(|r| paths_equal(r, root)) {
            debug!(root = %root.display(), "unwatching root");
            if let Err(error) = watcher.unwatch(Path::new(root)) {
                warn!(root = %root.display(), %error, "failed to unwatch root");
            }
        }
    }
    for root in new_roots {
        if is_remote_root(root) {
            continue;
        }
        if !old_roots.iter().any(|r| paths_equal(r, root)) {
            debug!(root = %root.display(), "watching root");
            if let Err(error) = watcher.watch(Path::new(root), RecursiveMode::Recursive) {
                warn!(root = %root.display(), %error, "failed to watch root");
            }
        }
    }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    normalize_key(a) == normalize_key(b)
}

fn root_for_path(roots: &[PathBuf], path: &Path) -> Option<PathBuf> {
    let key = normalize_key(path);
    roots
        .iter()
        .filter(|root| !is_remote_root(root))
        .find(|root| {
            let root_key = normalize_key(root);
            key == root_key
                || key
                    .strip_prefix(&root_key)
                    .is_some_and(|rest| rest.starts_with('/'))
        })
        .cloned()
}
