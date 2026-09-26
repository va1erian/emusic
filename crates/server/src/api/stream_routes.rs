//! Streaming endpoint: HTTP Range serving of every supported format.
//!
//! Standard audio is served with `206 Partial Content`; SID, tracker module
//! and MIDI files are served raw (with their specialized content type) so the
//! client keeps its native rendering and visualisation. In all cases the file
//! is resolved through the store and the path jail — a client can only name a
//! track id.

use std::path::PathBuf;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio_util::io::ReaderStream;

use crate::api::error::ApiError;
use crate::api::range::{RangeError, parse_range};
use crate::audit;
use crate::auth::middleware::{AuthDevice, ClientIp};
use crate::error::ServerError;
use crate::scanner::formats;
use crate::state::AppState;

/// `GET /api/v1/tracks/{id}/stream`
pub async fn stream(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    AuthDevice(device): AuthDevice,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let db = state.db.clone();
    let lookup = id.clone();
    let track = tokio::task::spawn_blocking(move || db.track_by_id(&lookup))
        .await
        .map_err(|_| ApiError::internal())??;
    let Some(track) = track else {
        return Err(ApiError::not_found());
    };

    let path = match state.roots.resolve(track.root_index, &track.relative_path) {
        Ok(path) => path,
        Err(error @ (ServerError::PathRejected(_) | ServerError::PathEscape { .. })) => {
            audit::path_violation(&ip.to_string(), &device.id, &id);
            return Err(error.into());
        }
        Err(error) => return Err(error.into()),
    };

    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        _ => return Err(ApiError::not_found()),
    };
    let len = metadata.len();
    let mime = formats::classify(&track.format)
        .map(|info| info.mime)
        .unwrap_or("application/octet-stream");
    let etag = format!("\"{}\"", track.hash);

    if headers.get(header::RANGE).is_none() && matches_etag(&headers, &etag) {
        return Ok((
            StatusCode::NOT_MODIFIED,
            [(header::ETAG, etag.as_str())],
            Body::empty(),
        )
            .into_response());
    }

    let requested = headers
        .get(header::RANGE)
        .map(|value| value.to_str().unwrap_or_default());

    match requested {
        None => serve(&path, 0, len.saturating_sub(1), len, mime, &etag, false).await,
        Some(raw) => match parse_range(raw, len) {
            Ok((start, end)) => serve(&path, start, end, len, mime, &etag, true).await,
            Err(RangeError::Unsatisfiable) => Ok(unsatisfiable(len)),
            Err(RangeError::Invalid) => Err(ApiError::bad_request("invalid range")),
        },
    }
}

async fn serve(
    path: &PathBuf,
    start: u64,
    end: u64,
    len: u64,
    mime: &str,
    etag: &str,
    partial: bool,
) -> Result<Response, ApiError> {
    let length = if len == 0 { 0 } else { end - start + 1 };
    let body = if length == 0 {
        Body::empty()
    } else {
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|_| ApiError::not_found())?;
        file.seek(SeekFrom::Start(start))
            .await
            .map_err(|_| ApiError::internal())?;
        let reader = file.take(length);
        Body::from_stream(ReaderStream::new(reader))
    };

    let mut builder = Response::builder()
        .status(if partial {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, length.to_string())
        .header(header::ETAG, etag);
    if partial {
        builder = builder.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
    }
    builder.body(body).map_err(|_| ApiError::internal())
}

fn unsatisfiable(len: u64) -> Response {
    (
        StatusCode::RANGE_NOT_SATISFIABLE,
        [(header::CONTENT_RANGE, format!("bytes */{len}"))],
        Body::empty(),
    )
        .into_response()
}

fn matches_etag(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|candidate| candidate.trim() == etag))
}
