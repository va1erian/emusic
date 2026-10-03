//! Starred-track sync endpoints (#513).
//!
//! One server-wide set shared by every paired device. Reads support
//! `If-None-Match` against an ETag of the starred-set version; every effective
//! change is audited and broadcast as `starred_changed` on the WebSocket.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde::{Deserialize, Serialize};
use tower_http::limit::RequestBodyLimitLayer;

use crate::api::error::ApiError;
use crate::auth::middleware::{AuthDevice, ClientIp};
use crate::db::starred::StarredUpdate;
use crate::scan::ServerEvent;
use crate::state::AppState;

/// Maximum ids (star + unstar) accepted by one batch request.
pub const MAX_BATCH_IDS: usize = 5_000;

/// Body limit for the batch route: room for [`MAX_BATCH_IDS`] 64-hex ids,
/// independent of the (much smaller) global `security.max_body_bytes`.
pub const MAX_BATCH_BODY_BYTES: usize = 1024 * 1024;

/// `GET /api/v1/starred` response.
#[derive(Debug, Serialize)]
pub struct StarredSetView {
    /// Starred-set version (also the ETag).
    pub version: i64,
    /// Starred tracks that currently exist, newest first.
    pub tracks: Vec<StarredTrackView>,
}

/// One starred track.
#[derive(Debug, Serialize)]
pub struct StarredTrackView {
    /// Server track id.
    pub id: String,
    /// Unix timestamp (seconds) when it was starred.
    pub starred_at: i64,
}

/// `PUT`/`DELETE /api/v1/starred/{track_id}` response.
#[derive(Debug, Serialize)]
pub struct StarredVersionView {
    /// Starred-set version after the request.
    pub version: i64,
}

/// `POST /api/v1/starred/batch` request.
#[derive(Debug, Deserialize)]
pub struct StarredBatchRequest {
    /// Ids to star.
    #[serde(default)]
    pub star: Vec<String>,
    /// Ids to unstar (applied after `star`).
    #[serde(default)]
    pub unstar: Vec<String>,
}

/// `POST /api/v1/starred/batch` response.
#[derive(Debug, Serialize)]
pub struct StarredBatchView {
    /// Starred-set version after the batch.
    pub version: i64,
    /// Ids in `star` that are not in the library (skipped).
    pub unknown: Vec<String>,
}

/// The batch route, with its own body limit. Merge it into the main router
/// *after* the global body limit layer so that limit does not apply to it.
pub fn batch_router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/starred/batch", post(batch))
        .layer(RequestBodyLimitLayer::new(MAX_BATCH_BODY_BYTES))
}

/// `GET /api/v1/starred`
pub async fn list(
    State(state): State<AppState>,
    AuthDevice(_): AuthDevice,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let db = state.db.clone();
    let snapshot = tokio::task::spawn_blocking(move || db.starred())
        .await
        .map_err(|_| ApiError::internal())??;
    let etag = format!("\"{}\"", snapshot.version);
    if if_none_match(&headers, snapshot.version) {
        return Ok((StatusCode::NOT_MODIFIED, [(header::ETAG, etag)]).into_response());
    }
    let body = StarredSetView {
        version: snapshot.version,
        tracks: snapshot
            .entries
            .into_iter()
            .map(|entry| StarredTrackView {
                id: entry.track_id,
                starred_at: entry.starred_at,
            })
            .collect(),
    };
    Ok((
        [
            (header::ETAG, etag),
            (header::CACHE_CONTROL, "private, no-cache".to_string()),
        ],
        Json(body),
    )
        .into_response())
}

/// `PUT /api/v1/starred/{track_id}` — unknown track is a 404.
pub async fn star(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    AuthDevice(device): AuthDevice,
    Path(track_id): Path<String>,
) -> Result<Json<StarredVersionView>, ApiError> {
    let update = apply(&state, vec![track_id], Vec::new()).await?;
    if !update.unknown.is_empty() {
        return Err(ApiError::not_found());
    }
    publish(&state, &ip.to_string(), &device.id, &update);
    Ok(Json(StarredVersionView {
        version: update.version,
    }))
}

/// `DELETE /api/v1/starred/{track_id}` — idempotent.
pub async fn unstar(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    AuthDevice(device): AuthDevice,
    Path(track_id): Path<String>,
) -> Result<Json<StarredVersionView>, ApiError> {
    let update = apply(&state, Vec::new(), vec![track_id]).await?;
    publish(&state, &ip.to_string(), &device.id, &update);
    Ok(Json(StarredVersionView {
        version: update.version,
    }))
}

/// `POST /api/v1/starred/batch`. The body extractor comes last so an
/// unauthenticated request is rejected before its body is read.
pub async fn batch(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    AuthDevice(device): AuthDevice,
    Json(request): Json<StarredBatchRequest>,
) -> Result<Json<StarredBatchView>, ApiError> {
    if request.star.len() + request.unstar.len() > MAX_BATCH_IDS {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_BATCH_IDS} ids per batch"
        )));
    }
    let update = apply(&state, request.star, request.unstar).await?;
    publish(&state, &ip.to_string(), &device.id, &update);
    Ok(Json(StarredBatchView {
        version: update.version,
        unknown: update.unknown,
    }))
}

async fn apply(
    state: &AppState,
    star: Vec<String>,
    unstar: Vec<String>,
) -> Result<StarredUpdate, ApiError> {
    let db = state.db.clone();
    let update = tokio::task::spawn_blocking(move || db.apply_starred(&star, &unstar))
        .await
        .map_err(|_| ApiError::internal())??;
    Ok(update)
}

/// Audits and broadcasts an effective change; a no-op is silent.
fn publish(state: &AppState, client_ip: &str, device_id: &str, update: &StarredUpdate) {
    if !update.changed() {
        return;
    }
    state.audit.starred_changed(
        client_ip,
        device_id,
        update.starred.len(),
        update.unstarred.len(),
    );
    state.scan.broadcast(ServerEvent::StarredChanged {
        version: update.version,
    });
}

/// Whether `If-None-Match` names the current version (or is `*`).
fn if_none_match(headers: &HeaderMap, version: i64) -> bool {
    let current = version.to_string();
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|tag| tag.trim().trim_start_matches("W/").trim_matches('"'))
        .any(|tag| tag == "*" || tag == current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn if_none_match_accepts_quoted_weak_lists_and_star() {
        assert!(if_none_match(&headers("\"7\""), 7));
        assert!(if_none_match(&headers("W/\"7\""), 7));
        assert!(if_none_match(&headers("\"3\", \"7\""), 7));
        assert!(if_none_match(&headers("*"), 7));
        assert!(!if_none_match(&headers("\"6\""), 7));
        assert!(!if_none_match(&HeaderMap::new(), 7));
    }
}
