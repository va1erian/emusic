//! Shared application state handed to every request handler.

use std::sync::Arc;

use ipnet::IpNet;

use crate::auth::ServerKey;
use crate::config::Config;
use crate::db::Db;
use crate::error::Result;
use crate::render::{RenderCapabilities, RenderService};
use crate::scan::ScanCoordinator;
use crate::scanner::{Scanner, SongLengths};
use crate::security::{LibraryRoots, RateLimiter};
use crate::util::unix_now;

/// Immutable + shared runtime state.
#[derive(Clone)]
pub struct AppState {
    /// Validated configuration.
    pub config: Arc<Config>,
    /// SQLite store.
    pub db: Db,
    /// Server signing key.
    pub keys: Arc<ServerKey>,
    /// Authorized library roots.
    pub roots: Arc<LibraryRoots>,
    /// Pairing/auth rate limiter.
    pub rate: Arc<RateLimiter>,
    /// Trusted reverse-proxy networks.
    pub trusted: Arc<Vec<IpNet>>,
    /// Scan coordinator.
    pub scan: ScanCoordinator,
    /// SID song-length table (may be empty).
    pub songlengths: Arc<SongLengths>,
    /// Rendering service, or `None` when `render.enabled` is false.
    pub render: Option<Arc<RenderService>>,
    /// Capability metadata advertised with every track.
    pub render_caps: RenderCapabilities,
    /// Server start time (Unix seconds).
    pub started_at: i64,
}

impl AppState {
    /// Builds state from validated configuration, an open store and the
    /// server key. Non-fatal problems (e.g. a missing songlengths file) are
    /// logged and degrade gracefully.
    pub fn new(config: Config, db: Db, keys: ServerKey, songlengths: SongLengths) -> Result<Self> {
        let render = RenderService::new(
            &config.render,
            &config.server.data_dir,
            config.library.hvsc_songlengths_path.as_deref(),
        )?;
        let render_caps = match &render {
            Some(service) => service.capabilities(),
            None => RenderCapabilities::disabled(),
        };
        let config = Arc::new(config);
        let roots = Arc::new(LibraryRoots::new(&config.library.paths));
        let trusted = Arc::new(config.trusted_proxy_nets()?);
        let scanner = Scanner::new(config.library.paths.clone(), Arc::new(songlengths.clone()));
        let coordinator = ScanCoordinator::new(scanner, db.library_version()?);
        Ok(Self {
            config,
            db,
            keys: Arc::new(keys),
            roots,
            rate: Arc::new(RateLimiter::new()),
            trusted,
            scan: coordinator,
            songlengths: Arc::new(songlengths),
            render: render.map(Arc::new),
            render_caps,
            started_at: unix_now(),
        })
    }

    /// Access-token lifetime.
    pub fn token_ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.config.security.token_ttl_hours.saturating_mul(3600))
    }

    /// Maximum pairing attempts per minute.
    pub fn pairing_attempt_limit(&self) -> u32 {
        self.config.security.max_pairing_attempts_per_min
    }
}
