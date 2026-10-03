//! The [`AuditLog`] handle and its background writer.
//!
//! Request handlers must never wait on SQLite to record an audit event, and
//! a slow or failing database must never fail a request. Events are pushed
//! onto a bounded channel with `try_send`; one dedicated thread drains it and
//! inserts them in batches. When the queue is full the event is dropped and
//! counted, and the writer records an `audit_events_dropped` row with the
//! number lost as soon as it catches up.
//!
//! Events an anonymous client can trigger at will (failed auth, refused
//! pairing, rate limiting) go through [`AuditLog::record_per_client`], which
//! persists at most a few per client per minute so a flood cannot grow the
//! table without bound. The tracing line is always emitted.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::time::Duration;

use super::AUDIT_TARGET;
use super::record::{AuditLevel, AuditRecord};
use crate::db::Db;
use crate::security::RateLimiter;

/// Events buffered before new ones are dropped.
pub const QUEUE_CAPACITY: usize = 4096;

/// Rows inserted per transaction at most.
const MAX_BATCH: usize = 256;

/// Window for [`AuditLog::record_per_client`].
const PER_CLIENT_WINDOW: Duration = Duration::from_secs(60);

/// How long [`AuditLog::flush`] waits for the writer.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

/// A message to the writer thread.
pub(crate) enum Command {
    /// Persist this event.
    Record(AuditRecord),
    /// Acknowledge once everything queued before this has been written.
    Flush(mpsc::Sender<()>),
}

/// Records audit events to tracing and to the `audit_log` table.
///
/// Cheap to clone; every clone feeds the same writer. The writer thread exits
/// once the last clone is dropped and the queue is drained.
#[derive(Clone)]
pub struct AuditLog {
    inner: Arc<Inner>,
}

struct Inner {
    sink: Sink,
    dropped: Arc<AtomicU64>,
    throttle: RateLimiter,
    throttled: AtomicU64,
}

enum Sink {
    /// Tracing only.
    Disabled,
    /// Synchronous inserts, for short-lived CLI commands.
    Direct(Db),
    /// The bounded queue drained by the writer thread.
    Queue(SyncSender<Command>),
}

impl AuditLog {
    /// A log that only emits tracing events and persists nothing.
    pub fn disabled() -> Self {
        Self::with_sink(Sink::Disabled)
    }

    /// A log that inserts each event synchronously. Only for CLI commands,
    /// where there is no request to protect and the process exits right away.
    pub fn direct(db: Db) -> Self {
        Self::with_sink(Sink::Direct(db))
    }

    /// A log backed by a background writer thread inserting into `db`.
    pub fn spawn(db: Db) -> std::io::Result<Self> {
        let (log, receiver) = Self::queued(QUEUE_CAPACITY);
        let dropped = Arc::clone(&log.inner.dropped);
        std::thread::Builder::new()
            .name("emusic-audit".into())
            .spawn(move || run_writer(&db, &receiver, &dropped))?;
        Ok(log)
    }

    /// A queue-backed log without a writer, so tests can drive [`run_writer`].
    pub(crate) fn queued(capacity: usize) -> (Self, Receiver<Command>) {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        (Self::with_sink(Sink::Queue(sender)), receiver)
    }

    fn with_sink(sink: Sink) -> Self {
        Self {
            inner: Arc::new(Inner {
                sink,
                dropped: Arc::new(AtomicU64::new(0)),
                throttle: RateLimiter::new(),
                throttled: AtomicU64::new(0),
            }),
        }
    }

    /// Emits `record` to tracing and queues it for persistence. Never blocks
    /// and never fails.
    pub fn record(&self, record: AuditRecord) {
        emit_tracing(&record);
        match &self.inner.sink {
            Sink::Disabled => {}
            Sink::Direct(db) => {
                if let Err(error) = db.insert_audit_records(std::slice::from_ref(&record)) {
                    tracing::error!(%error, "cannot persist audit event");
                }
            }
            Sink::Queue(sender) => match sender.try_send(Command::Record(record)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => self.count_drop(),
                Err(TrySendError::Disconnected(_)) => {
                    tracing::error!("audit writer stopped; event not persisted");
                }
            },
        }
    }

    /// Like [`Self::record`], but persists at most `per_minute` events with
    /// this name per client address; the rest only reach tracing.
    pub fn record_per_client(&self, record: AuditRecord, per_minute: u32) {
        let key = format!("{}:{}", record.event, record.client_ip);
        if self
            .inner
            .throttle
            .check(&key, per_minute, PER_CLIENT_WINDOW)
        {
            self.record(record);
        } else {
            emit_tracing(&record);
            self.inner.throttled.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Events kept out of the table by [`Self::record_per_client`], since
    /// startup.
    pub fn throttled(&self) -> u64 {
        self.inner.throttled.load(Ordering::Relaxed)
    }

    /// Events dropped because the queue was full, since startup.
    pub fn dropped(&self) -> u64 {
        self.inner.dropped.load(Ordering::Relaxed)
    }

    /// Blocks until every event queued so far is written (or a few seconds
    /// pass). Call it from a blocking context.
    pub fn flush(&self) {
        let Sink::Queue(sender) = &self.inner.sink else {
            return;
        };
        let (ack, done) = mpsc::channel();
        if sender.send(Command::Flush(ack)).is_ok() {
            let _ = done.recv_timeout(FLUSH_TIMEOUT);
        }
    }

    fn count_drop(&self) {
        let total = self.inner.dropped.fetch_add(1, Ordering::Relaxed) + 1;
        // Warn on the first drop and then at powers of two, so a flood does
        // not turn into a flood of warnings.
        if total.is_power_of_two() {
            tracing::warn!(dropped = total, "audit queue full; events dropped");
        }
    }
}

fn emit_tracing(record: &AuditRecord) {
    let detail = record.detail_json();
    let device_id = record.device_id.as_deref();
    match record.level {
        AuditLevel::Info => tracing::info!(
            target: AUDIT_TARGET,
            event = record.event,
            client_ip = %record.client_ip,
            device_id,
            detail = %detail
        ),
        AuditLevel::Warn => tracing::warn!(
            target: AUDIT_TARGET,
            event = record.event,
            client_ip = %record.client_ip,
            device_id,
            detail = %detail
        ),
    }
}

/// Drains `receiver` into `db` in batches until every sender is gone.
pub(crate) fn run_writer(db: &Db, receiver: &Receiver<Command>, dropped: &AtomicU64) {
    let mut reported = 0u64;
    while let Ok(first) = receiver.recv() {
        let mut records = Vec::new();
        let mut acks = Vec::new();
        sort_command(first, &mut records, &mut acks);
        while records.len() < MAX_BATCH {
            match receiver.try_recv() {
                Ok(command) => sort_command(command, &mut records, &mut acks),
                Err(_) => break,
            }
        }

        write_batch(db, records, dropped, &mut reported);
        for ack in acks {
            let _ = ack.send(());
        }
    }
}

/// Inserts `records` plus an `audit_events_dropped` row for drops not yet
/// reported. `reported` only advances once that row is actually written, so
/// a failed insert leaves the drop count to be reported by the next batch.
pub(crate) fn write_batch(
    db: &Db,
    mut records: Vec<AuditRecord>,
    dropped: &AtomicU64,
    reported: &mut u64,
) {
    let total = dropped.load(Ordering::Relaxed);
    if total > *reported {
        records.push(
            AuditRecord::new("audit_events_dropped")
                .warn()
                .field("count", total - *reported),
        );
    }
    if records.is_empty() {
        return;
    }
    match db.insert_audit_records(&records) {
        Ok(_) => *reported = total,
        Err(error) => {
            tracing::error!(%error, lost = records.len(), "cannot persist audit events");
        }
    }
}

fn sort_command(
    command: Command,
    records: &mut Vec<AuditRecord>,
    acks: &mut Vec<mpsc::Sender<()>>,
) {
    match command {
        Command::Record(record) => records.push(record),
        Command::Flush(ack) => acks.push(ack),
    }
}
