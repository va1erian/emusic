//! Library sync and metadata routes.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use serde::Deserialize;
use std::sync::Arc;

use crate::AppState;
use crate::db::models::ServerTrack;
use crate::util::security::canonicalize_and_validate_path;

#[derive(Debug, Deserialize)]
pub struct SyncQuery {
    pub since_version: Option<i64>,
}

pub async fn sync_library(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<Vec<ServerTrack>>, (StatusCode, String)> {
    let since = query.since_version.unwrap_or(0);
    let tracks = state
        .db
        .list_tracks(since)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(tracks))
}

pub async fn track_meta(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ServerTrack>, (StatusCode, String)> {
    let track = state
        .db
        .get_track(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    Ok(Json(track))
}

pub async fn album_art(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let track = state
        .db
        .get_track(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    let file_path =
        canonicalize_and_validate_path(&state.config.library.paths, &track.relative_path)
            .map_err(|_| (StatusCode::NOT_FOUND, "File not found".to_string()))?;

    // Try finding cover art in same directory (folder.jpg, cover.jpg, etc.)
    if let Some(parent) = file_path.parent() {
        for cover_name in &["cover.jpg", "folder.jpg", "cover.png", "folder.png"] {
            let art_path = parent.join(cover_name);
            if art_path.is_file()
                && let Ok(bytes) = std::fs::read(&art_path)
            {
                let mime = mime_guess::from_path(&art_path).first_or_octet_stream();
                return Ok(([(header::CONTENT_TYPE, mime.to_string())], bytes));
            }
        }
    }

    Err((StatusCode::NOT_FOUND, "Cover art not found".to_string()))
}
