use std::sync::Arc;
use axum::{
    Router,
    extract::{State, Query, Path, ws::{WebSocket, WebSocketUpgrade}},
    response::{IntoResponse, Response, Json},
    routing::{get, post},
    http::{StatusCode, header, HeaderMap, HeaderValue},
    body::Body,
};
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use sha2::Digest;
use tracing::{info, warn};

use crate::db::{DbStore, PairedDevice};
use crate::util::security::{TokenManager, resolve_path_in_roots};
use crate::config::AppConfig;

pub struct AppState {
    pub db: Arc<DbStore>,
    pub token_mgr: Arc<TokenManager>,
    pub config: AppConfig,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(healthcheck))
        .route("/api/v1/auth/pair", post(pair_device))
        .route("/api/v1/library/sync", get(sync_library))
        .route("/api/v1/tracks/{id}/meta", get(get_track_meta))
        .route("/api/v1/tracks/{id}/stream", get(stream_track))
        .route("/api/v1/ws", get(ws_handler))
        .with_state(state)
}

async fn healthcheck() -> &'static str {
    "OK"
}

#[derive(Deserialize)]
struct PairRequest {
    pairing_code: String,
    device_name: String,
    public_key: String,
}

#[derive(Serialize)]
struct PairResponse {
    device_id: String,
    auth_token: String,
}

async fn pair_device(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PairRequest>,
) -> Result<Json<PairResponse>, (StatusCode, &'static str)> {
    let valid = state
        .db
        .consume_pairing_code(&payload.pairing_code)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    if !valid {
        warn!("Failed pairing attempt with code: {}", payload.pairing_code);
        return Err((StatusCode::UNAUTHORIZED, "Invalid or expired pairing code"));
    }

    let device_id = hex::encode(sha2::Sha256::digest(payload.public_key.as_bytes()))[..16].to_string();
    let now_str = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    let device = PairedDevice {
        id: device_id.clone(),
        name: payload.device_name,
        public_key: payload.public_key,
        paired_at: now_str.clone(),
        last_seen: Some(now_str),
        is_revoked: false,
    };

    state
        .db
        .register_device(&device)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to register device"))?;

    let token = state
        .token_mgr
        .generate_token(&device_id, state.config.security.token_ttl_hours * 3600);

    info!("Successfully paired new device: {} ({})", device.name, device.id);

    Ok(Json(PairResponse {
        device_id,
        auth_token: token,
    }))
}

fn authenticate_request(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<String, (StatusCode, &'static str)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

    if !auth_header.starts_with("Bearer ") {
        return Err((StatusCode::UNAUTHORIZED, "Invalid Authorization scheme"));
    }

    let token = &auth_header[7..];
    let claims = state
        .token_mgr
        .verify_token(token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token"))?;

    if let Ok(Some(device)) = state.db.get_device(&claims.device_id) {
        if device.is_revoked {
            return Err((StatusCode::FORBIDDEN, "Device has been revoked"));
        }
    } else {
        return Err((StatusCode::UNAUTHORIZED, "Unknown device"));
    }

    Ok(claims.device_id)
}

#[derive(Deserialize)]
struct SyncQuery {
    since_version: Option<u64>,
}

async fn sync_library(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<SyncQuery>,
) -> Response {
    if let Err(err) = authenticate_request(&headers, &state) {
        return err.into_response();
    }
    let since = query.since_version.unwrap_or(0);

    match state.db.get_tracks_since(since) {
        Ok(tracks) => Json(tracks).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read tracks").into_response(),
    }
}

async fn get_track_meta(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(err) = authenticate_request(&headers, &state) {
        return err.into_response();
    }

    match state.db.get_track_by_id(&id) {
        Ok(Some(track)) => Json(track).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Track not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    }
}

async fn stream_track(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if let Err(err) = authenticate_request(&headers, &state) {
        return err.into_response();
    }

    let track = match state.db.get_track_by_id(&id) {
        Ok(Some(t)) => t,
        Ok(None) => return (StatusCode::NOT_FOUND, "Track not found").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
    };

    let full_path = match resolve_path_in_roots(&state.config.library.paths, &track.relative_path) {
        Some(p) => p,
        None => return (StatusCode::FORBIDDEN, "Access denied / Invalid path").into_response(),
    };

    let file = match tokio::fs::File::open(&full_path).await {
        Ok(f) => f,
        Err(_) => return (StatusCode::NOT_FOUND, "Audio file not found").into_response(),
    };

    let mime = mime_guess::from_path(&full_path)
        .first_or_octet_stream()
        .to_string();

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        mime.parse().unwrap_or_else(|_| header::HeaderValue::from_static("application/octet-stream")),
    );

    response
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(err) = authenticate_request(&headers, &state) {
        return err.into_response();
    }
    ws.on_upgrade(handle_ws)
}

async fn handle_ws(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        if matches!(msg, axum::extract::ws::Message::Close(_)) {
            break;
        }
    }
}
