//! Data behind each admin page, shared by the HTML pages and `/admin/api/*`.

use serde::Serialize;

use super::stats::{self, FileSizes, PageStats, PairingCodeStats, TableCount, TombstoneStats};
use crate::activity::DeviceActivity;
use crate::api::auth_routes::DeviceView;
use crate::audit::{AuditQuery, AuditRow};
use crate::error::{Result, ServerError};
use crate::scan::ScanStatus;
use crate::state::AppState;
use crate::util::unix_now;

/// The overview page.
#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    /// Server version.
    pub version: &'static str,
    /// Start time (Unix seconds).
    pub started_at: i64,
    /// Seconds since start.
    pub uptime_secs: i64,
    /// `PRAGMA user_version`.
    pub schema_version: i64,
    /// Delta-sync library version.
    pub library_version: i64,
    /// Database file sizes.
    pub files: FileSizes,
    /// Rows per table.
    pub tables: Vec<TableCount>,
    /// Scan coordinator state.
    pub scan: ScanStatus,
    /// Rendition cache size in bytes, when rendering is enabled.
    pub render_cache_bytes: Option<u64>,
    /// Rendition cache cap in bytes, when rendering is enabled.
    pub render_cache_max_bytes: Option<u64>,
    /// Configured library roots.
    pub roots: Vec<String>,
    /// Devices active right now.
    pub active_devices: usize,
    /// Audit events dropped because the queue was full.
    pub audit_events_dropped: u64,
    /// Client-triggerable audit events kept out of the table by the
    /// per-client throttle (still in the log file).
    pub audit_events_throttled: u64,
}

/// The database page.
#[derive(Debug, Clone, Serialize)]
pub struct DatabaseReport {
    /// Rows per table.
    pub tables: Vec<TableCount>,
    /// Tombstones.
    pub tombstones: TombstoneStats,
    /// Pairing-code rows.
    pub pairing_codes: PairingCodeStats,
    /// Page accounting.
    pub pages: PageStats,
    /// Database file sizes.
    pub files: FileSizes,
    /// `PRAGMA quick_check` output.
    pub quick_check: Vec<String>,
}

/// One row of the active-users table.
#[derive(Debug, Clone, Serialize)]
pub struct ActiveDevice {
    /// Live activity.
    #[serde(flatten)]
    pub activity: DeviceActivity,
    /// Display label of `last_track_id`, when the track still exists.
    pub last_track: Option<String>,
}

/// The pairing page.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PairingStatus {
    /// Pairing-code rows by state.
    pub codes: PairingCodeStats,
    /// Lifetime of a newly generated code, in seconds.
    pub code_ttl_secs: u64,
}

/// One page of the audit log.
#[derive(Debug, Clone, Serialize)]
pub struct AuditPage {
    /// Rows, newest first.
    pub rows: Vec<AuditRow>,
    /// Cursor for the next (older) page, when there may be one.
    pub next_before_id: Option<i64>,
    /// Event names present in the log.
    pub events: Vec<String>,
}

/// Runs `work` on the blocking pool.
pub async fn blocking<T, F>(work: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ServerError::Io(std::io::Error::other(error)))?
}

/// Gathers the overview.
pub async fn overview(state: &AppState) -> Result<Overview> {
    let db = state.db.clone();
    let data_dir = state.config.server.data_dir.clone();
    let render = state.render.clone();
    let (schema_version, library_version, tables, files, cache) = blocking(move || {
        let cache = render
            .as_ref()
            .map(|service| service.cache().total_bytes().unwrap_or(0));
        Ok((
            stats::schema_version(&db)?,
            db.library_version()?,
            stats::table_counts(&db)?,
            stats::file_sizes(&data_dir),
            cache,
        ))
    })
    .await?;
    let now = unix_now();
    Ok(Overview {
        version: env!("CARGO_PKG_VERSION"),
        started_at: state.started_at,
        uptime_secs: now.saturating_sub(state.started_at),
        schema_version,
        library_version,
        files,
        tables,
        scan: state.scan.status().await,
        render_cache_bytes: cache,
        render_cache_max_bytes: state.render.as_ref().map(|s| s.cache().max_bytes()),
        roots: state
            .config
            .library
            .paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        active_devices: state.activity.active(now).len(),
        audit_events_dropped: state.audit.dropped(),
        audit_events_throttled: state.audit.throttled(),
    })
}

/// Gathers the database report, running `PRAGMA quick_check`.
pub async fn database(state: &AppState) -> Result<DatabaseReport> {
    let db = state.db.clone();
    let data_dir = state.config.server.data_dir.clone();
    blocking(move || {
        Ok(DatabaseReport {
            tables: stats::table_counts(&db)?,
            tombstones: stats::tombstone_stats(&db)?,
            pairing_codes: stats::pairing_code_stats(&db, unix_now())?,
            pages: stats::page_stats(&db)?,
            files: stats::file_sizes(&data_dir),
            quick_check: stats::check_integrity(&db, false)?,
        })
    })
    .await
}

/// Active devices with their last track resolved to a label.
pub async fn active(state: &AppState) -> Result<Vec<ActiveDevice>> {
    let devices = state.activity.active(unix_now());
    let db = state.db.clone();
    blocking(move || {
        devices
            .into_iter()
            .map(|activity| {
                let last_track = match &activity.last_track_id {
                    Some(id) => stats::track_label(&db, id)?,
                    None => None,
                };
                Ok(ActiveDevice {
                    activity,
                    last_track,
                })
            })
            .collect()
    })
    .await
}

/// Every paired device, newest first.
pub async fn devices(state: &AppState) -> Result<Vec<DeviceView>> {
    let db = state.db.clone();
    let devices = blocking(move || db.list_devices()).await?;
    Ok(devices.into_iter().map(DeviceView::from).collect())
}

/// Pairing-code state.
pub async fn pairing(state: &AppState) -> Result<PairingStatus> {
    let db = state.db.clone();
    let codes = blocking(move || stats::pairing_code_stats(&db, unix_now())).await?;
    Ok(PairingStatus {
        codes,
        code_ttl_secs: state.config.security.pairing_code_ttl_secs,
    })
}

/// One page of the audit log. Flushes the writer first so the page includes
/// events raised a moment ago (such as the action that led here).
pub async fn audit(state: &AppState, query: AuditQuery) -> Result<AuditPage> {
    let db = state.db.clone();
    let audit = state.audit.clone();
    blocking(move || {
        audit.flush();
        let rows = db.audit_rows(&query)?;
        let full_page = rows.len() as u32 >= query.limit.max(1);
        let next_before_id = full_page.then(|| rows.last().map(|row| row.id)).flatten();
        Ok(AuditPage {
            rows,
            next_before_id,
            events: db.audit_event_names()?,
        })
    })
    .await
}
