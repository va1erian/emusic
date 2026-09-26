//! Library metadata, delta sync, album art and SID song-length endpoints.

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::api::error::ApiError;
use crate::audit;
use crate::auth::middleware::{AuthDevice, ClientIp};
use crate::db::models::{SyncDeltaView, TrackView};
use crate::error::ServerError;
use crate::scanner::art;
use crate::state::AppState;

/// Query for the delta-sync endpoint.
#[derive(Debug, Deserialize)]
pub struct SyncQuery {
    /// The client's last known library version.
    pub since_version: Option<i64>,
}

/// `GET /api/v1/library/sync`
pub async fn sync(
    State(state): State<AppState>,
    AuthDevice(_): AuthDevice,
    Query(query): Query<SyncQuery>,
) -> Result<Json<SyncDeltaView>, ApiError> {
    let since = query.since_version.unwrap_or(0).max(0);
    let db = state.db.clone();
    let delta = tokio::task::spawn_blocking(move || db.sync_since(since))
        .await
        .map_err(|_| ApiError::internal())??;
    Ok(Json(delta.into()))
}

/// `GET /api/v1/tracks/{id}/meta`
pub async fn track_meta(
    State(state): State<AppState>,
    AuthDevice(_): AuthDevice,
    Path(id): Path<String>,
) -> Result<Json<TrackView>, ApiError> {
    let db = state.db.clone();
    let track = tokio::task::spawn_blocking(move || db.track_by_id(&id))
        .await
        .map_err(|_| ApiError::internal())??;
    track
        .map(|track| Json(TrackView::from(&track)))
        .ok_or_else(ApiError::not_found)
}

/// `GET /api/v1/albums/{id}/art`
pub async fn album_art(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    AuthDevice(device): AuthDevice,
    Path(album_id): Path<String>,
) -> Result<Response, ApiError> {
    let db = state.db.clone();
    let roots = state.roots.clone();
    let lookup = album_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        let Some(track) = db.art_candidate_for_album(&lookup)? else {
            return Ok(None);
        };
        let path = roots.resolve(track.root_index, &track.relative_path)?;
        Ok::<_, ServerError>(art::extract(&path))
    })
    .await
    .map_err(|_| ApiError::internal())?;

    match result {
        Ok(Some(artwork)) => Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, artwork.mime),
                (header::CACHE_CONTROL, "private, max-age=3600".to_string()),
            ],
            Body::from(artwork.bytes),
        )
            .into_response()),
        Ok(None) => Err(ApiError::not_found()),
        // Only a genuine escape is a security event; a missing/stale file is
        // ordinary and must not pollute the audit log.
        Err(error @ ServerError::PathEscape { .. }) => {
            audit::path_violation(&ip.to_string(), &device.id, &album_id);
            Err(error.into())
        }
        Err(error) => Err(error.into()),
    }
}

/// `GET /api/v1/sid/songlengths` — served as HVSC `Songlengths.md5` text so
/// the client's existing parser can consume it unchanged.
pub async fn songlengths(State(state): State<AppState>, AuthDevice(_): AuthDevice) -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        state.songlengths.to_hvsc_text(),
    )
        .into_response()
}
