//! Debounced sync after a star change (#516).
//!
//! Starring only writes the local flag; a few seconds after the last change
//! the library backend is asked to run a remote sync, which pushes it. A
//! burst of clicks yields one sync. The timer runs on a small helper thread,
//! started on first use, that posts [`Update::StarSyncDue`] (waking the UI)
//! when it fires; it exits when the debouncer is dropped.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

use super::super::{Update, Updates};

/// How long after the last star change the sync runs.
pub(crate) const STAR_SYNC_DELAY: Duration = Duration::from_secs(3);

/// Coalesces star changes into one delayed [`Update::StarSyncDue`].
pub(crate) struct StarSyncDebouncer {
    delay: Duration,
    tx: Option<Sender<()>>,
}

impl StarSyncDebouncer {
    /// A debouncer firing `delay` after the last [`poke`](Self::poke).
    pub(crate) fn new(delay: Duration) -> Self {
        Self { delay, tx: None }
    }

    /// Records a star change, (re)starting the delay.
    pub(crate) fn poke(&mut self, updates: &Updates) {
        if let Some(tx) = &self.tx
            && tx.send(()).is_ok()
        {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let delay = self.delay;
        let updates = updates.clone();
        let spawned = std::thread::Builder::new()
            .name("star-sync-debounce".into())
            .spawn(move || {
                // Each round: wait for a change, then until `delay` passes
                // without another one.
                while rx.recv().is_ok() {
                    loop {
                        match rx.recv_timeout(delay) {
                            Ok(()) => continue,
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    if updates.send(Update::StarSyncDue).is_err() {
                        return;
                    }
                }
            });
        match spawned {
            Ok(_) => {
                // The thread is waiting for its first change; the channel
                // buffers it, so this send cannot be lost.
                let _ = tx.send(());
                self.tx = Some(tx);
            }
            Err(error) => tracing::warn!(%error, "could not start the star sync timer"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::waker::WakerHandle;

    #[test]
    fn a_burst_of_changes_fires_once_after_the_delay() {
        let (tx, rx) = mpsc::channel();
        let updates = Updates::new(tx, WakerHandle::default());
        let mut debouncer = StarSyncDebouncer::new(Duration::from_millis(150));
        let started = Instant::now();
        for _ in 0..5 {
            debouncer.poke(&updates);
            std::thread::sleep(Duration::from_millis(20));
        }
        let fired = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the debounced sync fires");
        assert!(matches!(fired, Update::StarSyncDue));
        assert!(started.elapsed() >= Duration::from_millis(150 + 80));
        assert!(
            rx.recv_timeout(Duration::from_millis(400)).is_err(),
            "one burst fires once"
        );
        // A later change fires again on the same thread.
        debouncer.poke(&updates);
        let fired = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("fires again");
        assert!(matches!(fired, Update::StarSyncDue));
    }
}
