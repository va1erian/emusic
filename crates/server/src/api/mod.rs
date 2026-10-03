//! HTTP API: router, health check and shared error type.

pub mod auth_routes;
pub mod error;
pub mod library_routes;
pub mod range;
pub mod render_routes;
pub mod starred_routes;
pub mod stream_routes;
pub mod ws;

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::{delete, get, post, put};
use serde_json::{Value, json};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// Builds the complete application router.
pub fn router(state: AppState) -> Router {
    let max_body = state.config.security.max_body_bytes;
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/auth/pair", post(auth_routes::pair))
        .route("/api/v1/auth/refresh", post(auth_routes::refresh))
        .route("/api/v1/devices", get(auth_routes::list_devices))
        .route(
            "/api/v1/devices/pairing-codes",
            post(auth_routes::create_pairing_code),
        )
        .route("/api/v1/devices/{id}", delete(auth_routes::revoke))
        .route("/api/v1/library/sync", get(library_routes::sync))
        .route("/api/v1/tracks/{id}/meta", get(library_routes::track_meta))
        .route("/api/v1/tracks/{id}/stream", get(stream_routes::stream))
        .route("/api/v1/tracks/{id}/render", get(render_routes::render))
        .route("/api/v1/albums/{id}/art", get(library_routes::album_art))
        .route("/api/v1/sid/songlengths", get(library_routes::songlengths))
        .route("/api/v1/starred", get(starred_routes::list))
        .route(
            "/api/v1/starred/{track_id}",
            put(starred_routes::star).delete(starred_routes::unstar),
        )
        .route("/api/v1/ws", get(ws::ws))
        .layer(RequestBodyLimitLayer::new(max_body))
        // Merged after the global limit: the batch route carries its own.
        .merge(starred_routes::batch_router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// `GET /api/v1/health` — unauthenticated liveness probe. Deliberately
/// exposes no library state: it only confirms the process is serving.
async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "started_at": state.started_at,
    }))
}
