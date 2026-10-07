//! The decode worker pool behind [`ThumbCache`](super::ThumbCache).
//!
//! Jobs wait on a stack, not a queue: the newest request is decoded first. A
//! fast scroll through a gallery requests every tile it passes, and with a
//! FIFO the covers the user lands on would wait behind all of them (each one a
//! network read on a NAS-hosted library). The tiles scrolled past are still
//! decoded afterwards, which warms the on-disk thumbnail cache for the way
//! back.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use super::{DecodeFn, Rgba8Image};
use crate::waker::{Waker as _, WakerHandle};

/// A decode request handed to the worker pool.
pub(super) struct Job {
    pub(super) key: u64,
    pub(super) source: PathBuf,
    pub(super) fallback_dir: Option<PathBuf>,
    pub(super) waker: WakerHandle,
}

/// A finished decode, sent back to the UI thread.
pub(super) struct Finished {
    pub(super) key: u64,
    pub(super) image: Option<Rgba8Image>,
}

/// The pending jobs, newest last, and whether the pool was dropped.
#[derive(Default)]
struct Stack {
    jobs: Vec<Job>,
    closed: bool,
}

/// The stack shared with the workers, and the condition they sleep on.
#[derive(Default)]
struct Shared {
    stack: Mutex<Stack>,
    ready: Condvar,
}

/// A fixed set of decode threads fed newest-first. Dropping the pool stops
/// them after their current decode, abandoning the jobs still pending.
pub(super) struct Pool {
    shared: Arc<Shared>,
    finished: Receiver<Finished>,
}

impl Pool {
    /// Starts `workers` threads decoding through `decode`.
    pub(super) fn spawn(workers: usize, decode: &DecodeFn) -> Self {
        let shared = Arc::new(Shared::default());
        let (done_tx, finished) = mpsc::channel();
        for _ in 0..workers {
            let shared = Arc::clone(&shared);
            let done = done_tx.clone();
            let decode = Arc::clone(decode);
            std::thread::spawn(move || worker(&shared, &done, &decode));
        }
        Self { shared, finished }
    }

    /// Queues `job` ahead of every job already pending.
    pub(super) fn submit(&self, job: Job) {
        self.shared
            .stack
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .jobs
            .push(job);
        self.shared.ready.notify_one();
    }

    /// The next finished decode, if any.
    pub(super) fn try_recv(&self) -> Option<Finished> {
        self.finished.try_recv().ok()
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        let mut stack = self
            .shared
            .stack
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        stack.closed = true;
        stack.jobs.clear();
        drop(stack);
        self.shared.ready.notify_all();
    }
}

/// Decodes the newest pending job until the pool is dropped.
fn worker(shared: &Shared, done: &Sender<Finished>, decode: &DecodeFn) {
    loop {
        let job = {
            let mut stack = shared.stack.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                if stack.closed {
                    return;
                }
                if let Some(job) = stack.jobs.pop() {
                    break job;
                }
                stack = shared
                    .ready
                    .wait(stack)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let image = decode(&job.source, job.fallback_dir.as_deref());
        if done
            .send(Finished {
                key: job.key,
                image,
            })
            .is_err()
        {
            return;
        }
        job.waker.wake();
    }
}
