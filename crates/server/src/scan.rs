//! Async coordination of library scans.
//!
//! Only one scan runs at a time. The coordinator owns the broadcast channel
//! the WebSocket fan-out subscribes to, and exposes a cheap snapshot of the
//! current status for the REST API.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tokio::sync::RwLock;
use tokio::sync::broadcast;

use crate::db::Db;
use crate::error::{Result, ServerError};
use crate::scanner::{DEFAULT_BATCH_SIZE, ScanReport, ScanUpdate, Scanner};
use crate::util::unix_now;

/// Events broadcast to WebSocket subscribers.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// A scan started.
    ScanStarted {
        /// Unix timestamp (seconds).
        at: i64,
        /// Number of roots being scanned.
        roots: usize,
    },
    /// A scan made progress.
    ScanProgress {
        /// Root index currently being scanned.
        root_index: usize,
        /// Running file count.
        files_found: u64,
    },
    /// A scan finished.
    ScanFinished {
        /// Files discovered.
        files_found: u64,
        /// Rows changed.
        changed: u64,
        /// Rows deleted.
        deleted: u64,
        /// Whether the scan was partial.
        partial: bool,
        /// Duration in milliseconds.
        elapsed_ms: u64,
    },
    /// The library version advanced.
    LibraryChanged {
        /// New library version.
        version: i64,
    },
}

/// A snapshot of scan state for the status endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct ScanStatus {
    /// Whether a scan is in progress.
    pub running: bool,
    /// Files seen in the current or last scan.
    pub files_found: u64,
    /// Number of library roots.
    pub roots: usize,
    /// Last completed scan's summary, if any.
    pub last_report: Option<LastScan>,
    /// Current library version.
    pub library_version: i64,
}

/// Summary of the last completed scan.
#[derive(Debug, Clone, Serialize)]
pub struct LastScan {
    /// Files discovered.
    pub files_found: u64,
    /// Rows changed.
    pub changed: u64,
    /// Rows deleted.
    pub deleted: u64,
    /// Whether the scan was partial.
    pub partial: bool,
    /// Duration in milliseconds.
    pub elapsed_ms: u64,
    /// Unix timestamp (seconds) when the scan finished.
    pub finished_at: i64,
}

/// Owns the scanner and its runtime state.
#[derive(Clone)]
pub struct ScanCoordinator {
    scanner: Arc<Scanner>,
    status: Arc<RwLock<ScanStatus>>,
    events: broadcast::Sender<ServerEvent>,
    running: Arc<AtomicBool>,
}

impl ScanCoordinator {
    /// Creates a coordinator with no scan in progress.
    pub fn new(scanner: Scanner, library_version: i64) -> Self {
        let (events, _) = broadcast::channel(256);
        let roots = scanner.root_count();
        Self {
            scanner: Arc::new(scanner),
            status: Arc::new(RwLock::new(ScanStatus {
                running: false,
                files_found: 0,
                roots,
                last_report: None,
                library_version,
            })),
            events,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Subscribes to scan events.
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.events.subscribe()
    }

    /// The current status snapshot.
    pub async fn status(&self) -> ScanStatus {
        self.status.read().await.clone()
    }

    /// Whether a scan is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Requests a scan if none is running. Returns whether one was started.
    pub fn trigger(&self, db: Db) -> bool {
        if self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        let coordinator = self.clone();
        tokio::spawn(async move {
            if let Err(error) = coordinator.run(db).await {
                tracing::error!(%error, "library scan failed");
            }
            coordinator.running.store(false, Ordering::Release);
        });
        true
    }

    /// Runs a scan to completion, bypassing the single-scan guard. Used by
    /// startup code and tests that need determinism.
    pub async fn scan_once(&self, db: Db) -> Result<ScanReport> {
        self.run(db).await
    }

    /// Starts a periodic scan loop. `interval` of zero disables it.
    pub fn start_periodic(&self, db: Db, interval: Duration) {
        if interval.is_zero() {
            return;
        }
        let coordinator = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            ticker.tick().await;
            loop {
                ticker.tick().await;
                coordinator.trigger(db.clone());
            }
        });
    }

    async fn run(&self, db: Db) -> Result<ScanReport> {
        let roots = self.scanner.root_count();
        let _ = self.events.send(ServerEvent::ScanStarted {
            at: unix_now(),
            roots,
        });
        {
            let mut status = self.status.write().await;
            status.running = true;
            status.files_found = 0;
        }

        let scanner = Arc::clone(&self.scanner);
        let events = self.events.clone();
        let status = Arc::clone(&self.status);
        let scan_db = db.clone();
        let report = tokio::task::spawn_blocking(move || {
            let mut on_update = |update: ScanUpdate| {
                let _ = events.send(ServerEvent::ScanProgress {
                    root_index: update.root_index,
                    files_found: update.files_found,
                });
                if let Ok(mut status) = status.try_write() {
                    status.files_found = update.files_found;
                }
            };
            scanner.scan(&scan_db, DEFAULT_BATCH_SIZE, &mut on_update)
        })
        .await
        .map_err(|error| ServerError::Metadata(format!("scan task failed: {error}")))??;

        let version = db.library_version()?;
        {
            let mut status = self.status.write().await;
            status.running = false;
            status.files_found = report.files_found;
            status.library_version = version;
            status.last_report = Some(LastScan {
                files_found: report.files_found,
                changed: report.tracks_changed,
                deleted: report.tracks_deleted,
                partial: report.partial,
                elapsed_ms: report.elapsed_ms,
                finished_at: unix_now(),
            });
        }
        let _ = self.events.send(ServerEvent::ScanFinished {
            files_found: report.files_found,
            changed: report.tracks_changed,
            deleted: report.tracks_deleted,
            partial: report.partial,
            elapsed_ms: report.elapsed_ms,
        });
        let _ = self.events.send(ServerEvent::LibraryChanged { version });
        crate::audit::scan_finished(
            report.files_found,
            report.tracks_changed,
            report.tracks_deleted,
            report.partial,
            report.elapsed_ms,
        );
        Ok(report)
    }
}
