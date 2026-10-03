//! Server assembly and lifecycle.

use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use axum_server::tls_rustls::RustlsConfig;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::audit::{Actor, AuditLog, AuditQuery, AuditRow, CLI_SOURCE, SYSTEM_SOURCE, retention};
use crate::auth::ServerKey;
use crate::auth::keys::key_path;
use crate::config::Config;
use crate::db::Db;
use crate::error::{Result, ServerError};
use crate::scanner::SongLengths;
use crate::state::AppState;
use crate::util::unix_now;
use crate::{admin, api};

/// Grace period for in-flight requests on shutdown.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(10);

/// Opens the store, loads the server key and song lengths, returning shared
/// state ready to serve. Does not start listening or scanning.
pub fn build_state(config: Config) -> Result<AppState> {
    let db = Db::open(&config.server.data_dir)?;
    let keys = ServerKey::load_or_create(&key_path(&config.server.data_dir))?;
    let songlengths = load_songlengths(&config);
    let state = AppState::new(config, db, keys, songlengths)?;
    if let Some(migration) = state.db.migration() {
        state.audit.migration_applied(SYSTEM_SOURCE, migration);
    }
    Ok(state)
}

fn load_songlengths(config: &Config) -> SongLengths {
    let Some(path) = &config.library.hvsc_songlengths_path else {
        return SongLengths::empty();
    };
    match SongLengths::load(path) {
        Ok(table) => {
            tracing::info!(path = %path.display(), tunes = table.len(), "loaded song lengths");
            table
        }
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "song lengths unavailable, continuing");
            SongLengths::empty()
        }
    }
}

/// Runs the HTTP(S) server until `shutdown` resolves.
pub async fn serve(
    state: AppState,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let addr = listen_addr(&state.config)?;
    let app = api::router(state.clone());

    // One shutdown signal fans out to both listeners.
    let (stop, stopped) = watch::channel(false);
    tokio::spawn(async move {
        shutdown.await;
        let _ = stop.send(true);
    });
    // Bind the admin listener first so a bad admin config fails startup
    // before anything is served.
    let admin = spawn_admin(&state, stopped.clone()).await?;

    state.scan.trigger(state.db.clone());
    state.scan.start_periodic(
        state.db.clone(),
        Duration::from_secs(state.config.library.scan_interval_secs),
    );
    retention::spawn_daily(
        state.db.clone(),
        state.audit.clone(),
        state.config.audit.retention_days,
    );
    let shutdown = wait_for_stop(stopped);

    if state.config.tls_enabled() {
        let tls = RustlsConfig::from_pem_file(
            state.config.security.tls_cert.trim(),
            state.config.security.tls_key.trim(),
        )
        .await
        .map_err(|error| ServerError::Config(format!("cannot load TLS material: {error}")))?;

        tracing::info!(%addr, "listening with direct TLS");
        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();
        tokio::spawn(async move {
            shutdown.await;
            shutdown_handle.graceful_shutdown(Some(SHUTDOWN_GRACE));
        });
        axum_server::bind_rustls(addr, tls)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(|error| ServerError::Io(std::io::Error::other(error)))?;
    } else {
        let listener = TcpListener::bind(addr).await?;
        tracing::info!(%addr, "listening on plain HTTP (terminate TLS at the proxy)");
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown)
        .await?;
    }
    if let Some(admin) = admin {
        let _ = admin.await;
    }
    let audit = state.audit.clone();
    let _ = tokio::task::spawn_blocking(move || audit.flush()).await;
    Ok(())
}

/// Resolves once the shared stop flag is raised (or its sender is gone).
async fn wait_for_stop(mut stopped: watch::Receiver<bool>) {
    let _ = stopped.wait_for(|stop| *stop).await;
}

/// Binds and serves the admin router on its own listener, when enabled.
/// Never mounted on the public router; plain HTTP, meant for loopback or an
/// SSH tunnel.
async fn spawn_admin(
    state: &AppState,
    stopped: watch::Receiver<bool>,
) -> Result<Option<JoinHandle<()>>> {
    let config = &state.config.admin;
    if !config.enabled {
        tracing::info!("admin listener disabled");
        return Ok(None);
    }
    let addr = admin::listen_addr(config)?;
    let listener = TcpListener::bind(addr).await.map_err(|error| {
        ServerError::Config(format!("cannot bind admin listener on {addr}: {error}"))
    })?;
    tracing::info!(
        %addr,
        auth = if config.requires_auth() { "basic" } else { "none (loopback only)" },
        "admin listener ready"
    );
    let app = admin::router(state.clone());
    Ok(Some(tokio::spawn(async move {
        let served = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(wait_for_stop(stopped))
        .await;
        if let Err(error) = served {
            tracing::error!(%error, "admin listener failed");
        }
    })))
}

/// Initializes logging, builds state and serves until a shutdown signal.
pub async fn run(config: Config) -> Result<()> {
    let _guard = crate::audit::init(&config.server.data_dir)?;
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "emusic-server starting"
    );
    let state = build_state(config)?;
    serve(state, shutdown_signal()).await
}

/// Resolves the configured host/port into a socket address.
pub fn listen_addr(config: &Config) -> Result<SocketAddr> {
    let host = config.server.host.trim();
    let port = config.server.port;
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }
    format!("{host}:{port}").parse().map_err(|error| {
        ServerError::Config(format!(
            "cannot parse listen address {host}:{port}: {error}"
        ))
    })
}

/// Resolves on Ctrl-C (all platforms) or SIGTERM (Unix).
pub async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    tracing::info!("shutdown signal received");
}

/// Opens the store for a CLI command, with synchronous auditing.
fn open_for_cli(config: &Config) -> Result<(Db, AuditLog)> {
    let db = Db::open(&config.server.data_dir)?;
    let audit = AuditLog::direct(db.clone());
    if let Some(migration) = db.migration() {
        audit.migration_applied(CLI_SOURCE, migration);
    }
    Ok((db, audit))
}

/// Generates and prints a pairing code, for the `pair` CLI subcommand.
pub fn print_pairing_code(config: &Config, ttl_secs: u64) -> Result<String> {
    let (db, audit) = open_for_cli(config)?;
    let key = ServerKey::load_or_create(&key_path(&config.server.data_dir))?;
    let code = crate::auth::pairing::generate_pairing_code(&db, &key, ttl_secs, unix_now())?;
    audit.pairing_code_created(CLI_SOURCE, &Actor::Cli, ttl_secs);
    Ok(code)
}

/// Lists paired devices, for the `devices` CLI subcommand.
pub fn list_devices(config: &Config) -> Result<Vec<crate::db::models::Device>> {
    let (db, _audit) = open_for_cli(config)?;
    db.list_devices()
}

/// Revokes a device, for the `revoke` CLI subcommand.
pub fn revoke_device(config: &Config, id: &str) -> Result<bool> {
    let (db, audit) = open_for_cli(config)?;
    let revoked = db.revoke_device(id)?;
    if revoked {
        audit.device_revoked(CLI_SOURCE, id, &Actor::Cli);
    }
    Ok(revoked)
}

/// Recent audit rows, newest first, for the `audit` CLI subcommand.
pub fn list_audit(config: &Config, limit: u32, event: Option<String>) -> Result<Vec<AuditRow>> {
    let (db, _audit) = open_for_cli(config)?;
    db.audit_rows(&AuditQuery {
        event,
        limit,
        ..AuditQuery::default()
    })
}
