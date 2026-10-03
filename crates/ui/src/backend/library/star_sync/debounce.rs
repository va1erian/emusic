//! Debounced sync after a star change (#516).
//!
//! Starring only writes the local flag; a few seconds after the last change
//! the library backend is asked to run a remote sync, which pushes it. A
//! burst of clicks yields one sync. The timer runs on a small helper thread,
//! started on first use, that posts [`Update::StarSyncDue`] (waking the UI)
//! when it fires; it exits when the debouncer is dropped.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
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
            .spawn(move || run_timer(&rx, delay, &updates));
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

/// The timer thread: each round waits for a change, then until `delay`
/// passes without another one, and posts one [`Update::StarSyncDue`]. Returns
/// when the debouncer (the sender) or the update receiver is gone.
fn run_timer(rx: &Receiver<()>, delay: Duration, updates: &Updates) {
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
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::waker::WakerHandle;

    #[test]
    fn a_queued_burst_fires_once_and_a_later_change_fires_again() {
        let (update_tx, update_rx) = mpsc::channel();
        let updates = Updates::new(update_tx, WakerHandle::default());
        let (tx, rx) = mpsc::channel();
        // The whole burst is queued before the timer runs, so it is one
        // burst whatever the scheduler does: no wall-clock dependency.
        for _ in 0..5 {
            tx.send(()).expect("queue a change");
        }
        let delay = Duration::from_millis(50);
        let timer = std::thread::spawn(move || run_timer(&rx, delay, &updates));

        let fired = update_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the debounced sync fires");
        assert!(matches!(fired, Update::StarSyncDue));
        tx.send(()).expect("a later change");
        let fired = update_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("fires again");
        assert!(matches!(fired, Update::StarSyncDue));

        // Dropping the sender stops the timer; nothing else was posted.
        drop(tx);
        timer.join().expect("the timer exits");
        assert!(update_rx.try_recv().is_err(), "one sync per burst");
    }

    #[test]
    fn poke_starts_the_timer_and_fires() {
        let (tx, rx) = mpsc::channel();
        let updates = Updates::new(tx, WakerHandle::default());
        let mut debouncer = StarSyncDebouncer::new(Duration::from_millis(50));
        let started = Instant::now();
        debouncer.poke(&updates);
        let fired = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the debounced sync fires");
        assert!(matches!(fired, Update::StarSyncDue));
        assert!(started.elapsed() >= Duration::from_millis(50));
    }
}
