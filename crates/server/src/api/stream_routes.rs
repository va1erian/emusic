//! Audio streaming and raw file delivery endpoints with HTTP Range 206 support.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::io::{Seek, SeekFrom};
use std::sync::Arc;

use crate::AppState;
use crate::util::security::canonicalize_and_validate_path;

pub async fn stream_track(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let track = state
        .db
        .get_track(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Track not found".to_string()))?;

    let file_path =
        canonicalize_and_validate_path(&state.config.library.paths, &track.relative_path).map_err(
            |_| {
                (
                    StatusCode::NOT_FOUND,
                    "File not found on server disk".to_string(),
                )
            },
        )?;

    let mut file = std::fs::File::open(&file_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let file_size = track.file_size;
    let format = track.format.to_lowercase();

    let mime_guess_str = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    let mime_type = match format.as_str() {
        "sid" | "psid" | "rsid" => "application/x-sid",
        "mod" | "s3m" | "xm" | "it" | "mo3" => "audio/x-mod",
        "mid" | "midi" => "audio/midi",
        _ => &mime_guess_str,
    }
    .to_string();

    // Specialized tracker/SID/MIDI formats: direct raw full binary transfer
    if is_specialized_format(&format) {
        let mut buffer = Vec::with_capacity(file_size as usize);
        use std::io::Read;
        file.read_to_end(&mut buffer)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        return Ok((
            [
                (header::CONTENT_TYPE, mime_type),
                (header::CONTENT_LENGTH, file_size.to_string()),
            ],
            buffer,
        )
            .into_response());
    }

    // Standard audio streams: HTTP Range 206 Partial Content support
    if let Some(range_header) = headers.get(header::RANGE).and_then(|v| v.to_str().ok())
        && let Some((start, end)) = parse_range_header(range_header, file_size)
    {
        let chunk_size = end - start + 1;
        file.seek(SeekFrom::Start(start))
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut buffer = vec![0u8; chunk_size as usize];
        use std::io::Read;
        file.read_exact(&mut buffer)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let content_range = format!("bytes {start}-{end}/{file_size}");
        let response = (
            StatusCode::PARTIAL_CONTENT,
            [
                (header::CONTENT_TYPE, mime_type),
                (header::ACCEPT_RANGES, "bytes".to_string()),
                (header::CONTENT_RANGE, content_range),
                (header::CONTENT_LENGTH, chunk_size.to_string()),
            ],
            buffer,
        )
            .into_response();

        return Ok(response);
    }

    // Default full stream delivery
    let mut buffer = Vec::with_capacity(file_size as usize);
    use std::io::Read;
    file.read_to_end(&mut buffer)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime_type),
            (header::ACCEPT_RANGES, "bytes".to_string()),
            (header::CONTENT_LENGTH, file_size.to_string()),
        ],
        buffer,
    )
        .into_response())
}

pub async fn get_sid_songlengths(
    State(state): State<Arc<AppState>>,
) -> Result<Response, (StatusCode, String)> {
    let path = state.config.library.hvsc_songlengths_path.as_ref().ok_or((
        StatusCode::NOT_FOUND,
        "Songlengths.md5 path not configured".to_string(),
    ))?;

    let bytes = std::fs::read(path).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            "Songlengths.md5 file not found".to_string(),
        )
    })?;

    Ok(([(header::CONTENT_TYPE, "text/plain")], bytes).into_response())
}

fn is_specialized_format(ext: &str) -> bool {
    matches!(
        ext,
        "sid" | "psid" | "rsid" | "mod" | "s3m" | "xm" | "it" | "mo3" | "mid" | "midi"
    )
}

fn parse_range_header(header: &str, file_size: u64) -> Option<(u64, u64)> {
    if !header.starts_with("bytes=") {
        return None;
    }
    let range_str = &header["bytes=".len()..];
    let mut parts = range_str.split('-');

    let start_str = parts.next()?;
    let end_str = parts.next()?;

    let start: u64 = if start_str.is_empty() {
        0
    } else {
        start_str.parse().ok()?
    };

    let end: u64 = if end_str.is_empty() {
        file_size.saturating_sub(1)
    } else {
        end_str.parse().ok()?
    };

    if start <= end && end < file_size {
        Some((start, end))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_header() {
        assert_eq!(parse_range_header("bytes=0-499", 1000), Some((0, 499)));
        assert_eq!(parse_range_header("bytes=500-", 1000), Some((500, 999)));
    }
}
