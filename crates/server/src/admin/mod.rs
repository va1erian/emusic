//! The admin page: a second listener with its own router.
//!
//! [`router`] is only ever served on the `[admin]` listener; the public router
//! in [`crate::api`] has no `/admin` route at all. The listener binds to
//! loopback by default, and [`crate::config::AdminConfig::validate`] refuses a
//! non-loopback bind without a token. Pages are server-rendered HTML with no
//! scripts; every response carries `Cache-Control: no-store`,
//! `X-Frame-Options: DENY` and a strict CSP. The same data is available as
//! JSON under `/admin/api/*`.

pub mod auth;
pub mod csrf;
pub mod data;
mod html;
mod json;
mod pages;
pub mod stats;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::error::{Result, ServerError};
use crate::state::AppState;
use csrf::CsrfKey;

/// Request bodies on the admin listener are tiny form posts.
const MAX_ADMIN_BODY_BYTES: usize = 16 * 1024;

/// No scripts, no frames, styles only from our own stylesheet, forms only to
/// ourselves.
const CONTENT_SECURITY_POLICY: &str = "default-src 'none'; style-src 'self'; img-src 'self'; \
     form-action 'self'; frame-ancestors 'none'; base-uri 'none'";

/// State shared by admin handlers.
#[derive(Clone)]
pub struct AdminState {
    /// The application state (same store, audit log and activity tracker as
    /// the public listener).
    pub app: AppState,
    /// Form-token key.
    pub csrf: Arc<CsrfKey>,
}

/// Builds the admin router. Serve it only on the admin listener.
pub fn router(app: AppState) -> Router {
    let state = AdminState {
        app,
        csrf: Arc::new(CsrfKey::generate()),
    };
    Router::new()
        .route("/", get(|| async { Redirect::to("/admin/") }))
        .route("/admin", get(|| async { Redirect::to("/admin/") }))
        .route("/admin/", get(pages::overview::page))
        .route("/admin/style.css", get(stylesheet))
        .route("/admin/active", get(pages::active::page))
        .route("/admin/devices", get(pages::devices::page))
        .route("/admin/devices/{id}/revoke", post(pages::devices::revoke))
        .route("/admin/pairing", get(pages::pairing::page))
        .route("/admin/pairing/codes", post(pages::pairing::generate))
        .route("/admin/audit", get(pages::audit::page))
        .route("/admin/database", get(pages::database::page))
        .route(
            "/admin/database/integrity-check",
            post(pages::database::integrity_check),
        )
        .route("/admin/api/overview", get(json::overview))
        .route("/admin/api/active", get(json::active))
        .route("/admin/api/devices", get(json::devices))
        .route("/admin/api/pairing", get(json::pairing))
        .route("/admin/api/audit", get(json::audit))
        .route("/admin/api/database", get(json::database))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") })
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ))
        .layer(middleware::from_fn(security_headers))
        .layer(RequestBodyLimitLayer::new(MAX_ADMIN_BODY_BYTES))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// The admin listener's socket address.
pub fn listen_addr(config: &crate::config::AdminConfig) -> Result<SocketAddr> {
    let ip = config
        .ip()
        .ok_or_else(|| ServerError::Config(format!("cannot parse admin.host {:?}", config.host)))?;
    Ok(SocketAddr::new(ip, config.port))
}

async fn stylesheet() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        html::STYLESHEET,
    )
        .into_response()
}

/// Adds the hardening headers to every admin response, including errors and
/// authentication challenges.
async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    let mut set = |name: HeaderName, value: &'static str| {
        headers.insert(name, HeaderValue::from_static(value));
    };
    set(header::CACHE_CONTROL, "no-store");
    set(header::X_FRAME_OPTIONS, "DENY");
    set(header::CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY);
    set(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(header::REFERRER_POLICY, "no-referrer");
    response
}

/// Logs `error` and renders a detail-free `500`.
fn internal_error(error: &ServerError) -> Response {
    tracing::error!(%error, "admin request failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}
