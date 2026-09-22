//! The primary instance's pipe listener: accepts connections from
//! secondary launches and batches messages that arrive close together.

use std::io::{self, BufRead, BufReader};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use interprocess::os::windows::named_pipe::{
    DuplexPipeStream, PipeListener, PipeListenerOptions, PipeMode, pipe_mode,
};

use super::message::IpcMessage;
use super::pipe_path;

/// Explorer spawns one process per selected file on "Open with"; messages
/// that arrive within this window of each other are merged into one.
const BATCH_WINDOW: Duration = Duration::from_millis(150);

type Stream = DuplexPipeStream<pipe_mode::Bytes>;

/// Windows error code returned by `CreateNamedPipeW` when
/// `FILE_FLAG_FIRST_PIPE_INSTANCE` is set and an instance already exists,
/// i.e. another process is already primary.
pub(super) const ERROR_ACCESS_DENIED: i32 = 5;

/// Owns the primary instance's pipe listener and delivers batched
/// [`IpcMessage`]s from secondary launches.
///
/// Messages are delivered on an internal channel; call [`Listener::try_recv`]
/// (e.g. once per UI frame, or right after the `waker` callback given to
/// [`super::SingleInstance::acquire`] fires) to drain it.
pub struct Listener {
    receiver: mpsc::Receiver<IpcMessage>,
    pipe_name: String,
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
    batch_thread: Option<JoinHandle<()>>,
}

impl Listener {
    /// Attempts to become the primary instance for `app_id` by creating the
    /// first (exclusive) instance of its named pipe. Fails with an
    /// `ERROR_ACCESS_DENIED` (5) `io::Error` if another process already
    /// holds it.
    pub(super) fn start(
        app_id: &str,
        waker: impl Fn() + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let pipe_name = pipe_path(app_id);
        let listener: PipeListener<pipe_mode::Bytes, pipe_mode::Bytes> = PipeListenerOptions::new()
            .path(pipe_name.as_str())
            .mode(PipeMode::Bytes)
            .create()?;
        let listener = Arc::new(listener);
        let stop = Arc::new(AtomicBool::new(false));
        let (raw_tx, raw_rx) = mpsc::channel::<IpcMessage>();
        let (batch_tx, batch_rx) = mpsc::channel::<IpcMessage>();

        let accept_thread = {
            let listener = Arc::clone(&listener);
            let stop = Arc::clone(&stop);
            thread::spawn(move || accept_loop(&listener, &stop, &raw_tx))
        };
        let batch_thread = thread::spawn(move || batch_loop(&raw_rx, &batch_tx, &waker));

        Ok(Self {
            receiver: batch_rx,
            pipe_name,
            stop,
            accept_thread: Some(accept_thread),
            batch_thread: Some(batch_thread),
        })
    }

    /// Returns the next batched message, if one has arrived, without
    /// blocking.
    pub fn try_recv(&self) -> Option<IpcMessage> {
        self.receiver.try_recv().ok()
    }

    /// Blocks up to `timeout` for the next batched message.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<IpcMessage> {
        self.receiver.recv_timeout(timeout).ok()
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // `PipeListener::accept` blocks; connecting a throwaway client is
        // the simplest way to unblock it so the accept thread can observe
        // `stop` and exit.
        let _ = Stream::connect_by_path(self.pipe_name.as_str());
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.batch_thread.take() {
            let _ = handle.join();
        }
    }
}

fn accept_loop(
    listener: &PipeListener<pipe_mode::Bytes, pipe_mode::Bytes>,
    stop: &AtomicBool,
    raw_tx: &mpsc::Sender<IpcMessage>,
) {
    while !stop.load(Ordering::SeqCst) {
        let stream = match listener.accept() {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if let Some(message) = read_message(&stream) {
            // Only fails once the batcher thread has shut down, in which
            // case there is nothing left to do but stop accepting too.
            if raw_tx.send(message).is_err() {
                break;
            }
        }
    }
}

/// Reads a single newline-delimited JSON [`IpcMessage`] from `stream`.
fn read_message(stream: &Stream) -> Option<IpcMessage> {
    let mut line = String::new();
    let mut reader = BufReader::new(stream);
    match reader.read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => serde_json::from_str(line.trim_end()).ok(),
        Err(_) => None,
    }
}

/// Collects raw messages arriving within [`BATCH_WINDOW`] of one another
/// into a single merged message, then hands it to `batch_tx` and invokes
/// `waker` so the UI can pick it up.
fn batch_loop(
    raw_rx: &mpsc::Receiver<IpcMessage>,
    batch_tx: &mpsc::Sender<IpcMessage>,
    waker: &(impl Fn() + Send + Sync + ?Sized),
) {
    loop {
        let Ok(mut batch) = raw_rx.recv() else {
            return;
        };
        loop {
            match raw_rx.recv_timeout(BATCH_WINDOW) {
                Ok(next) => batch.merge(next),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        if batch_tx.send(batch).is_err() {
            return;
        }
        waker();
    }
}
