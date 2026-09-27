//! `/render` endpoint: a specialized track rendered to a streamable codec.
//!
//! Android cannot run the desktop's native SID/module/MIDI engines, so it asks
//! the server to render a rendition (FLAC today) and plays that. The endpoint
//! reuses [`crate::api::stream_routes::serve_with_range`], so a cached rendition
//! supports `Accept-Ranges`, `206`, `ETag` and seeking exactly like `/stream`.
//!
//! # Status codes
//! - `404` when `render.enabled` is false, the track is unknown, or the format
//!   has no renderer (modules and MIDI today — these renderers are follow-ups).
//! - `400` for an unsupported `codec` or an out-of-range `subtune`.
//! - `500` when a supported format fails to render; the cause is logged, never
//!   disclosed.
//!
//! Rendering is synchronous here but always runs on
//! [`tokio::task::spawn_blocking`] and is bounded by the configured
//! `render.max_concurrent` permits, so it never blocks the async runtime. A
//! `202 Accepted`/`Retry-After` handshake is a scoped follow-up.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use emusic_render::{RenderError, RenderOptions, Renderer, RenditionKey};
use serde::Deserialize;

use crate::api::error::ApiError;
use crate::api::stream_routes;
use crate::audit;
use crate::auth::middleware::{AuthDevice, ClientIp};
use crate::error::ServerError;
use crate::render::{RenderService, rendition_key};
use crate::state::AppState;

/// Query for the render endpoint.
#[derive(Debug, Deserialize)]
pub struct RenderQuery {
    /// 1-based subtune; omitted renders the file's default subtune.
    pub subtune: Option<u16>,
    /// Output codec; defaults to the configured one.
    pub codec: Option<String>,
}

/// `GET /api/v1/tracks/{id}/render`
pub async fn render(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    AuthDevice(device): AuthDevice,
    Path(id): Path<String>,
    Query(query): Query<RenderQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let Some(service) = state.render.clone() else {
        return Err(ApiError::not_found());
    };
    let requested_codec = query.codec.as_deref().unwrap_or(service.codec().as_str());
    if !requested_codec.eq_ignore_ascii_case(service.codec().as_str()) {
        return Err(ApiError::bad_request("unsupported codec"));
    }

    let db = state.db.clone();
    let lookup = id.clone();
    let track = tokio::task::spawn_blocking(move || db.track_by_id(&lookup))
        .await
        .map_err(|_| ApiError::internal())??;
    let Some(track) = track else {
        return Err(ApiError::not_found());
    };

    // Only SID has a renderer today; module/MIDI are consistently `404`.
    if !state.render_caps.is_renderable(&track.format) {
        return Err(ApiError::not_found());
    }
    if query.subtune == Some(0) || query.subtune.is_some_and(|n| u32::from(n) > track.subtunes) {
        return Err(ApiError::bad_request("invalid subtune"));
    }

    let source = match state.roots.resolve(track.root_index, &track.relative_path) {
        Ok(path) => path,
        Err(error @ ServerError::PathEscape { .. }) => {
            audit::path_violation(&ip.to_string(), &device.id, &id);
            return Err(error.into());
        }
        Err(error) => return Err(error.into()),
    };

    // 0 stands for "the renderer's default subtune" in the stable cache key.
    let cache_subtune = query.subtune.unwrap_or(0);
    let etag = rendition_etag(&service, &track.id, cache_subtune);
    let options = RenderOptions {
        subtune: query.subtune,
        max_duration: None,
    };

    let cached = lookup_cached(service.clone(), track.id.clone(), cache_subtune).await?;
    let path = match cached {
        Some(path) => path,
        None => {
            render_into_cache(
                service.clone(),
                source,
                track.id.clone(),
                cache_subtune,
                options,
            )
            .await?
        }
    };

    let len = tokio::fs::metadata(&path)
        .await
        .map_err(|_| ApiError::internal())?
        .len();
    stream_routes::serve_with_range(&path, &headers, len, "audio/flac", &etag).await
}

/// A strong validator for a rendition: the renderer version is part of it, so
/// a changed algorithm yields a fresh ETag.
fn rendition_etag(service: &RenderService, track_id: &str, subtune: u16) -> String {
    let key = RenditionKey::new(
        track_id,
        subtune,
        service.codec(),
        service.sid().family(),
        service.sid().version(),
    );
    format!("\"{}\"", key.digest())
}

/// Looks up a cached rendition without taking a render permit.
async fn lookup_cached(
    service: Arc<RenderService>,
    track_id: String,
    subtune: u16,
) -> Result<Option<PathBuf>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let key = rendition_key(&service, &track_id, subtune);
        service.cache().get(&key)
    })
    .await
    .map_err(|_| ApiError::internal())?
    .map_err(render_failed)
}

/// Renders into the cache under a concurrency permit, then returns the path.
///
/// The cache is checked again once the permit is held, so concurrent requests
/// for the same rendition coalesce and only the first one renders. Everything
/// runs on the blocking pool; the async runtime only waits on the permit and
/// the join handle.
async fn render_into_cache(
    service: Arc<RenderService>,
    source: PathBuf,
    track_id: String,
    subtune: u16,
    options: RenderOptions,
) -> Result<PathBuf, ApiError> {
    let permit = service
        .permits()
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiError::internal())?;
    tokio::task::spawn_blocking(move || -> Result<PathBuf, RenderError> {
        let _permit = permit;
        let key = rendition_key(&service, &track_id, subtune);
        if let Some(cached) = service.cache().get(&key)? {
            return Ok(cached);
        }
        let pcm = service.sid().render(&source, &options)?;
        let bytes = emusic_render::flac::encode(&pcm)?;
        service.cache().put(&key, &bytes)
    })
    .await
    .map_err(|_| ApiError::internal())?
    .map_err(render_failed)
}

/// Logs the render failure and returns a detail-free `500`.
fn render_failed(error: RenderError) -> ApiError {
    tracing::error!(%error, "rendition failed");
    ApiError::internal()
}
